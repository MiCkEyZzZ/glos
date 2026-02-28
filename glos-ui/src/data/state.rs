use std::{collections::VecDeque, sync::Arc};

use chrono::{DateTime, Utc};
use parking_lot::RwLock;

/// Статус подключения источника данных
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Mock,
    Live,
    Replay,
}

impl ConnectionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Disconnected => "Отключено",
            Self::Mock => "Генератор тестовых данных",
            Self::Live => "Живой поток",
            Self::Replay => "Воспроизведение файла",
        }
    }

    pub fn color(&self) -> egui::Color32 {
        match self {
            Self::Disconnected => egui::Color32::from_rgb(180, 50, 50),
            Self::Mock => egui::Color32::from_rgb(200, 150, 50),
            Self::Live => egui::Color32::from_rgb(50, 180, 50),
            Self::Replay => egui::Color32::from_rgb(50, 150, 200),
        }
    }
}

/// Данные о спутнике
#[derive(Debug, Clone)]
pub struct Satellite {
    pub id: String,
    pub constellation: String,
    pub cn0: f32,       // dBHz
    pub elevation: f32, // градусы
    pub azimuth: f32,   // градусы
    pub doppler: f32,   // Гц
    pub used_in_fix: bool,
}

/// Спектральные данные
#[derive(Debug, Clone)]
pub struct SignalData {
    pub timestamp: DateTime<Utc>,
    pub frequency_mhz: f32,
    pub sample_rate_mhz: f32,
    pub fft_data: Vec<f32>,            // Мощность в dB
    pub waterfall: VecDeque<Vec<f32>>, // История для waterfall
}

impl SignalData {
    pub fn new(
        freq: f32,
        sample_rate: f32,
        fft_size: usize,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            frequency_mhz: freq,
            sample_rate_mhz: sample_rate,
            fft_data: vec![0.0; fft_size],
            waterfall: VecDeque::with_capacity(256),
        }
    }

    pub fn push_waterfall(
        &mut self,
        data: Vec<f32>,
    ) {
        if self.waterfall.len() >= 256 {
            self.waterfall.pop_front();
        }
        self.waterfall.push_back(data);
    }
}

/// Системные метрики
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub bandwidth_mhz: f32,
    pub buffer_usage: f32,
    pub packets_per_sec: u32,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            bandwidth_mhz: 0.0,
            buffer_usage: 0.0,
            packets_per_sec: 0,
        }
    }
}

/// Основное состояние приложения
pub struct AppState {
    pub status: ConnectionStatus,
    pub satellites: Vec<Satellite>,
    pub signal_data: SignalData,
    pub metrics: SystemMetrics,

    // GNSS fix данные
    pub position_lat: f64,
    pub position_lon: f64,
    pub altitude: f32,
    pub velocity: f32,
    pub hdop: f32,
    pub pdop: f32,

    // История CN0 для графиков
    pub cn0_history: VecDeque<(DateTime<Utc>, f32)>,

    // Логи
    pub log_messages: VecDeque<(DateTime<Utc>, String)>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            satellites: Vec::new(),
            signal_data: SignalData::new(1575.42, 4.0, 512),
            metrics: SystemMetrics::default(),
            position_lat: 55.7512,
            position_lon: 37.6184,
            altitude: 150.0,
            velocity: 0.0,
            hdop: 1.0,
            pdop: 1.5,
            cn0_history: VecDeque::with_capacity(300),
            log_messages: VecDeque::with_capacity(1000),
        }
    }
}

impl AppState {
    pub fn new() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self::default()))
    }

    pub fn add_log(
        &mut self,
        message: String,
    ) {
        if self.log_messages.len() >= 1000 {
            self.log_messages.pop_front();
        }
        self.log_messages.push_back((Utc::now(), message));
    }

    pub fn avg_cn0(&self) -> f32 {
        if self.satellites.is_empty() {
            return 0.0;
        }
        self.satellites.iter().map(|s| s.cn0).sum::<f32>() / self.satellites.len() as f32
    }

    pub fn satellite_count(&self) -> usize {
        self.satellites.len()
    }

    pub fn used_satellites(&self) -> usize {
        self.satellites.iter().filter(|s| s.used_in_fix).count()
    }
}
