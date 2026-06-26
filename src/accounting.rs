// Phase 5: Cost Optimization
// C_total = C_llm_tokens + C_user_attention + C_risk
// + route selection (script vs LLM) + Bayesian classifier for predictions

use crate::config::{Config, CostConfig};
use crate::types::{AggregatedCosts, CostCounters, RiskLevel};

/// Разбивка стоимости по компонентам.
#[derive(Debug, Clone)]
pub struct CostBreakdown {
    /// Стоимость LLM токенов (в $)
    pub llm_cost: f64,
    /// Стоимость внимания пользователя (в $)
    pub attention_cost: f64,
    /// Стоимость риска (в $)
    pub risk_cost: f64,
    /// Итого: C_llm + C_attention + C_risk
    pub total: f64,
}

impl CostBreakdown {
    pub fn zero() -> Self {
        Self {
            llm_cost: 0.0,
            attention_cost: 0.0,
            risk_cost: 0.0,
            total: 0.0,
        }
    }

    pub fn format(&self) -> String {
        format!(
            "  LLM tokens:   ${:.6}\n  Attention:     ${:.6}\n  Risk:          ${:.6}\n  ─────────────────\n  Total:         ${:.6}",
            self.llm_cost, self.attention_cost, self.risk_cost, self.total
        )
    }
}

/// Решение о маршруте: скрипт (дёшево) vs LLM (гибко).
#[derive(Debug, Clone, PartialEq)]
pub enum RouteDecision {
    /// Использовать скрипт (экономия).
    Script(String),
    /// Использовать LLM.
    Llm,
}

// ============================================================================
// Формула C_total
// ============================================================================

/// C_llm_tokens: стоимость LLM токенов.
pub fn compute_llm_cost(tokens: u64, config: &CostConfig) -> f64 {
    (tokens as f64 / 1000.0) * config.token_price_per_1k
}

/// C_user_attention: стоимость внимания пользователя.
/// Учитывает delay (ожидание ответа) + overhead (подтверждение).
pub fn compute_attention_cost(
    duration_ms: u64,
    delay_ms: u64,
    risk: &RiskLevel,
    config: &CostConfig,
) -> f64 {
    let total_sec = (duration_ms + delay_ms) as f64 / 1000.0;
    let base = total_sec * config.attention_cost_per_sec;
    // Overhead за подтверждение для рискованных операций
    let confirm_overhead = if risk_needs_confirmation(risk) {
        config.attention_cost_per_sec * 3.0 // ~3 сек на прочтение и ввод y/N
    } else {
        0.0
    };
    base + confirm_overhead
}

/// C_risk: стоимость риска (потенциальный ущерб от выполнения команды).
pub fn compute_risk_cost(risk: &RiskLevel, config: &CostConfig) -> f64 {
    config.risk_cost_weight(risk)
}

/// Полная стоимость: C_total = C_llm + C_attention + C_risk.
pub fn compute_total_cost(
    counters: &CostCounters,
    risk: &RiskLevel,
    llm_delay_ms: u64,
    config: &CostConfig,
) -> CostBreakdown {
    let llm = compute_llm_cost(counters.llm_cost.tokens, config);
    let attention = compute_attention_cost(
        counters.execution_cost.duration_ms,
        llm_delay_ms,
        risk,
        config,
    );
    let risk = compute_risk_cost(risk, config);
    CostBreakdown {
        llm_cost: llm,
        attention_cost: attention,
        risk_cost: risk,
        total: llm + attention + risk,
    }
}

/// Нужно ли подтверждение для данного уровня риска (влияет на attention cost).
fn risk_needs_confirmation(risk: &RiskLevel) -> bool {
    matches!(
        risk,
        RiskLevel::LocalWrite
            | RiskLevel::Destructive
            | RiskLevel::NetworkWrite
            | RiskLevel::CredentialAccess
            | RiskLevel::Financial
    )
}

// ============================================================================
// Выбор маршрута: скрипт vs LLM
// ============================================================================

/// Выбрать маршрут исполнения запроса.
/// - Если есть синоним → script (самый дешёвый)
/// - Если есть скрипт с высоким confidence → script
/// - Иначе → LLM
pub fn select_route(
    _request: &str,
    synonym_script_id: Option<&str>,
    _config: &Config,
) -> RouteDecision {
    // 1. Синоним (выученный из успешных LLM-ответов) — самый дешёвый
    if let Some(script_id) = synonym_script_id {
        return RouteDecision::Script(script_id.to_string());
    }

    // 2. Проверяем ScriptEngine (core/user/learned скрипты)
    //    (это делается в try_script_command, здесь только эвристика)
    //    Пока пропускаем — try_script_command сделает это в основном потоке.

    // 3. По умолчанию — LLM
    RouteDecision::Llm
}

/// Оценка экономии от использования скрипта вместо LLM.
pub fn estimated_savings(_request: &str, _script_id: &str, config: &CostConfig) -> f64 {
    // LLM cost: примерно 100 токенов на типичный запрос
    let llm_est = compute_llm_cost(100, config);
    // Script cost: практически 0 (локальное выполнение)
    let script_est = 0.0;
    // Плюс экономия внимания (script не требует ожидания LLM)
    let attention_saved = config.attention_cost_per_sec * 5.0; // ~5 сек экономии
    (llm_est - script_est) + attention_saved
}

// ============================================================================
// Bayesian classifier для точности предсказаний
// ============================================================================

/// Простой байесовский классификатор для proactive-предсказаний.
/// Использует prior + likelihood из наблюдаемых переходов.
#[derive(Debug, Clone)]
pub struct BayesianPredictor {
    /// Prior P(request): сколько раз каждый запрос встречался.
    priors: std::collections::HashMap<String, u64>,
    /// Likelihood P(next | prev): переходы prev -> {next: count}.
    likelihoods: std::collections::HashMap<String, std::collections::HashMap<String, u64>>,
    /// Всего наблюдений.
    total: u64,
}

impl BayesianPredictor {
    pub fn new() -> Self {
        Self {
            priors: std::collections::HashMap::new(),
            likelihoods: std::collections::HashMap::new(),
            total: 0,
        }
    }

    /// Добавить наблюдение (последовательность запросов).
    pub fn observe(&mut self, requests: &[String]) {
        for req in requests {
            *self.priors.entry(req.clone()).or_insert(0) += 1;
            self.total += 1;
        }
        for window in requests.windows(2) {
            let prev = &window[0];
            let next = &window[1];
            self.likelihoods
                .entry(prev.clone())
                .or_default()
                .entry(next.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
    }

    /// Предсказать следующий запрос с байесовской вероятностью.
    /// P(next | prev) = count(prev -> next) / count(prev)
    pub fn predict(&self, last_request: &str) -> Option<(String, f64)> {
        let inner = self.likelihoods.get(last_request)?;
        let total_from_state: u64 = inner.values().sum();
        if total_from_state == 0 {
            return None;
        }
        // Maximum likelihood: самый частый следующий запрос
        let (best_next, best_count) = inner.iter().max_by_key(|(_, &c)| c)?;
        let confidence = *best_count as f64 / total_from_state as f64;
        Some((best_next.clone(), confidence))
    }

    /// Количество уникальных запросов в модели.
    pub fn vocabulary_size(&self) -> usize {
        self.priors.len()
    }

    /// Количество наблюдений.
    pub fn total_observations(&self) -> u64 {
        self.total
    }
}

impl Default for BayesianPredictor {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Aggregation (legacy)
// ============================================================================

/// Суммирует cost_counters по типам.
pub fn aggregate(counters: &[CostCounters]) -> AggregatedCosts {
    let mut total = AggregatedCosts::default();
    for c in counters {
        total.total_duration_ms += c.execution_cost.duration_ms;
        total.total_commands += c.execution_cost.commands_executed;
        total.total_bytes_read += c.execution_cost.bytes_read;
        total.total_bytes_written += c.execution_cost.bytes_written;
        total.total_tokens += c.llm_cost.tokens;
        total.total_llm_duration_ms += c.llm_cost.duration_ms;
        if c.cache_cost.hit {
            total.cache_hits += 1;
        } else {
            total.cache_misses += 1;
        }
        total.total_storage_written += c.storage_cost.bytes_written;
        total.total_storage_read += c.storage_cost.bytes_read;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CostConfig {
        CostConfig::default()
    }

    fn make_counters(exec_ms: u64, commands: u64, tokens: u64, cache_hit: bool) -> CostCounters {
        CostCounters {
            execution_cost: crate::types::ExecutionCost {
                duration_ms: exec_ms,
                commands_executed: commands,
                ..Default::default()
            },
            llm_cost: crate::types::LlmCost {
                tokens,
                ..Default::default()
            },
            cache_cost: crate::types::CacheCost {
                hit: cache_hit,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_llm_cost_zero_tokens() {
        let c = test_config();
        assert_eq!(compute_llm_cost(0, &c), 0.0);
    }

    #[test]
    fn test_llm_cost_1000_tokens() {
        let c = test_config();
        // default price: $0.01 per 1K tokens → 1000 tokens = $0.01
        assert!((compute_llm_cost(1000, &c) - 0.01).abs() < 1e-6);
    }

    #[test]
    fn test_attention_cost_basic() {
        let c = test_config();
        let cost = compute_attention_cost(1000, 2000, &RiskLevel::ReadOnly, &c);
        // 3 sec total * $0.001/sec = $0.003
        assert!((cost - 0.003).abs() < 1e-6);
    }

    #[test]
    fn test_attention_cost_with_confirm() {
        let c = test_config();
        let cost = compute_attention_cost(1000, 2000, &RiskLevel::Destructive, &c);
        // 3 sec + 3 sec overhead = 6 sec = $0.006
        assert!((cost - 0.006).abs() < 1e-6);
    }

    #[test]
    fn test_risk_cost_readonly() {
        let c = test_config();
        assert_eq!(compute_risk_cost(&RiskLevel::ReadOnly, &c), 0.0);
    }

    #[test]
    fn test_risk_cost_destructive() {
        let c = test_config();
        assert_eq!(compute_risk_cost(&RiskLevel::Destructive, &c), 10.0);
    }

    #[test]
    fn test_total_cost() {
        let c = test_config();
        let counters = make_counters(1000, 1, 500, false);
        let breakdown = compute_total_cost(&counters, &RiskLevel::LocalWrite, 2000, &c);
        // C_llm = 500/1000 * 0.01 = 0.005
        // C_attention = (1+2) sec * 0.001 + 0.003 (confirm) = 0.006
        // C_risk = 1.0 (LocalWrite)
        // total = 0.005 + 0.006 + 1.0 = 1.011
        assert!((breakdown.total - 1.011).abs() < 1e-6);
    }

    #[test]
    fn test_select_route_synonym() {
        let config = Config::default();
        let route = select_route("list files", Some("ls_script"), &config);
        assert_eq!(route, RouteDecision::Script("ls_script".to_string()));
    }

    #[test]
    fn test_select_route_llm() {
        let config = Config::default();
        let route = select_route("unknown request", None, &config);
        assert_eq!(route, RouteDecision::Llm);
    }

    #[test]
    fn test_bayesian_predictor_basic() {
        let mut bp = BayesianPredictor::new();
        let reqs: Vec<String> = vec![
            "list files".into(),
            "show details".into(),
            "list files".into(),
            "show details".into(),
        ];
        bp.observe(&reqs);
        let (pred, conf) = bp.predict("list files").unwrap();
        assert_eq!(pred, "show details");
        assert!((conf - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_bayesian_predictor_no_data() {
        let bp = BayesianPredictor::new();
        assert!(bp.predict("anything").is_none());
    }

    #[test]
    fn test_estimated_savings() {
        let c = test_config();
        let savings = estimated_savings("list files", "ls_script", &c);
        // llm_est = 100/1000 * 0.01 = 0.001
        // attention_saved = 0.001 * 5 = 0.005
        // total = 0.001 + 0.005 = 0.006
        assert!(savings > 0.0);
        assert!(savings < 0.01);
    }

    #[test]
    fn test_cost_breakdown_format() {
        let b = CostBreakdown {
            llm_cost: 0.01,
            attention_cost: 0.005,
            risk_cost: 0.0,
            total: 0.015,
        };
        let s = b.format();
        assert!(s.contains("LLM tokens"));
        assert!(s.contains("Attention"));
        assert!(s.contains("Risk"));
        assert!(s.contains("Total"));
    }

    #[test]
    fn test_aggregate() {
        let counters = vec![
            make_counters(100, 2, 50, true),
            make_counters(200, 3, 30, false),
        ];
        let result = aggregate(&counters);
        assert_eq!(result.total_duration_ms, 300);
        assert_eq!(result.total_commands, 5);
        assert_eq!(result.total_tokens, 80);
        assert_eq!(result.cache_hits, 1);
        assert_eq!(result.cache_misses, 1);
    }
}
