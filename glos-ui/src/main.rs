//! GLOS-UI - интерфейс для GNSS/SDR анализа

fn main() -> eframe::Result<()> {
    env_logger::init();
    glos_ui::run()
}
