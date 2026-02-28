use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use chrono::Utc;
use parking_lot::RwLock;
use rand::Rng;

use crate::data::{AppState, ConnectionStatus, Satellite, SystemMetrics};

pub struct MockDataGenerator {
    state: Arc<RwLock<AppState>>,
    running: Arc<AtomicBool>,
}

impl MockDataGenerator {
    pub fn new(state: Arc<RwLock<AppState>>) -> Self {
        Self {
            state,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        self.running.store(true, Ordering::SeqCst);

        let state = Arc::clone(&self.state);
        let running_flag = Arc::clone(&self.running);

        // Логируем старт
        {
            let mut s = state.write();
            s.add_log("Запуск генератора тестовых данных...".to_string());
        }

        thread::spawn(move || {
            let mut rng = rand::rng();
            let mut time = 0.0f32;

            while running_flag.load(Ordering::SeqCst) {
                {
                    let mut state = state.write();

                    // Обновляем статус
                    state.status = ConnectionStatus::Mock;

                    // Генерируем спутники
                    state.satellites = Self::generate_satellites(&mut rng, time);

                    // Обновляем CN0 историю
                    let avg_cn0 = state.avg_cn0();
                    state.cn0_history.push_back((Utc::now(), avg_cn0));
                    if state.cn0_history.len() > 300 {
                        state.cn0_history.pop_front();
                    }

                    // Генерируем FFT данные
                    let fft_data = Self::generate_fft(&mut rng, time);
                    state.signal_data.fft_data = fft_data.clone();
                    state.signal_data.push_waterfall(fft_data);
                    state.signal_data.timestamp = Utc::now();

                    // Обновляем метрики
                    state.metrics = SystemMetrics {
                        cpu_usage: 25.0 + rng.random::<f32>() * 15.0,
                        bandwidth_mhz: 4.0,
                        buffer_usage: 45.0 + rng.random::<f32>() * 20.0,
                        packets_per_sec: 800 + rng.random::<u32>() % 200,
                    };

                    // Обновляем позицию (небольшой дрейф)
                    state.position_lat += (rng.random::<f64>() - 0.5) * 0.00001;
                    state.position_lon += (rng.random::<f64>() - 0.5) * 0.00001;
                    state.velocity = 0.1 + rng.random::<f32>() * 0.3;
                    state.hdop = 0.8 + rng.random::<f32>() * 0.5;

                    // Логи
                    if rng.random::<f32>() < 0.05 {
                        let messages = [
                            "Получено 1024 сэмпла",
                            "Решения обновлены",
                            "Спутник получен",
                            "Обработка корреляций",
                        ];
                        let random_index = rng.random_range(0..messages.len());
                        let msg = messages[random_index];
                        state.add_log(msg.to_string());
                    }
                } // lock released here

                time += 0.05;
                thread::sleep(Duration::from_millis(50));
            }

            // Обновляем статус при остановке
            {
                let mut state = state.write();
                state.status = ConnectionStatus::Disconnected;
                state.add_log("Генератор тестовых данных остановлен".to_string());
            }
        });
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn generate_satellites(
        rng: &mut impl Rng,
        time: f32,
    ) -> Vec<Satellite> {
        let constellations = [
            ("GPS", "G", 12),
            ("ГЛОНАСС", "R", 8),
            ("Галилео", "E", 6),
            ("Бэйдоу", "C", 5),
        ];

        let mut satellites = Vec::new();

        for (const_name, prefix, count) in constellations {
            for i in 1..=count {
                let phase = time * 0.1 + i as f32 * 0.5;

                satellites.push(Satellite {
                    id: format!("{prefix}{i:02}"),
                    constellation: const_name.to_string(),
                    cn0: 30.0 + 10.0 * (phase.sin() + 1.0) + rng.random::<f32>() * 3.0,
                    elevation: 15.0 + 60.0 * (phase.cos() + 1.0) / 2.0,
                    azimuth: ((i as f32 * 360.0 / count as f32) + time * 5.0) % 360.0,
                    doppler: -500.0 + (phase * 1.5).sin() * 800.0,
                    used_in_fix: rng.random::<f32>() > 0.3,
                });
            }
        }

        satellites
    }

    fn generate_fft(
        rng: &mut impl Rng,
        time: f32,
    ) -> Vec<f32> {
        let size = 512;
        let mut fft = Vec::with_capacity(size);

        for i in 0..size {
            let freq = i as f32 / size as f32;

            // Базовый шум
            let mut power = -80.0 + rng.random::<f32>() * 10.0;

            // Добавляем несколько пиков (сигналы)
            for peak in &[0.25, 0.5, 0.75] {
                let dist = (freq - peak).abs();
                if dist < 0.05 {
                    power += 40.0 * (1.0 - dist / 0.05);
                }
            }

            // Временная модуляция
            power += 5.0 * (time + freq * 10.0).sin();

            fft.push(power);
        }

        fft
    }
}
