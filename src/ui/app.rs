use dioxus::prelude::*;

pub fn run() {
    dioxus::desktop::launch(app);
}

fn app() -> Element {
    rsx! {
        div {
            style: "
                display: flex;
                flex-direction: column;
                height: 100vh;
                font-family: 'Courier New', monospace;
                background: #1e1e1e;
                color: #d4d4d4;
                padding: 8px;
            ",
            div {
                style: "
                    font-size: 18px;
                    font-weight: bold;
                    color: #569cd6;
                    padding: 8px 0;
                    border-bottom: 1px solid #333;
                ",
                "terio"
            }
            div {
                style: "
                    flex: 1;
                    overflow-y: auto;
                    padding: 8px 0;
                    font-size: 14px;
                    white-space: pre-wrap;
                ",
                "Добро пожаловать в terio.\n\nВведите запрос или команду.\n\n> "
            }
            input {
                style: "
                    width: 100%;
                    background: #2d2d2d;
                    border: 1px solid #333;
                    color: #d4d4d4;
                    padding: 8px;
                    font-family: 'Courier New', monospace;
                    font-size: 14px;
                    outline: none;
                ",
                placeholder: "Введите команду (terio run -- ...) или запрос..."
            }
        }
    }
}
