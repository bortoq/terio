// Accounting: cost_counters + aggregate + compute_attention_cost заглушка.

use crate::types::{AggregatedCosts, CostCounters};

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

/// Заглушка compute_attention_cost. В MVP всегда возвращает 0.0.
/// В будущем будет учитывать user_sec, tokens, duration и другие cost_counters.
pub fn compute_attention_cost(_counters: &CostCounters) -> f64 {
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_counters(exec_ms: u64, commands: u64, tokens: u64, cache_hit: bool) -> CostCounters {
        CostCounters {
            execution_cost: ExecutionCost {
                duration_ms: exec_ms,
                commands_executed: commands,
                ..ExecutionCost::default()
            },
            llm_cost: LlmCost {
                tokens,
                ..LlmCost::default()
            },
            cache_cost: CacheCost {
                hit: cache_hit,
                ..CacheCost::default()
            },
            ..CostCounters::default()
        }
    }

    #[test]
    fn test_aggregate_empty() {
        let result = aggregate(&[]);
        assert_eq!(result.total_duration_ms, 0);
        assert_eq!(result.cache_hits, 0);
    }

    #[test]
    fn test_aggregate_sums() {
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

    #[test]
    fn test_attention_cost_stub() {
        let c = make_counters(100, 1, 50, false);
        assert_eq!(compute_attention_cost(&c), 0.0);
    }
}
