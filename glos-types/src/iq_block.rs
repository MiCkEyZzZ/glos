/// Блок IQ данных (переменный размер)
#[derive(Debug, Clone)]
pub struct IqBlock {
    /// Метка времени блока в наносекундах (для точности)
    pub timestamp_ns: u64,
    /// Количество IQ выборок в блоке
    pub sample_count: u32,
    /// Данные IQ выборок (формат зависит от заголовка)
    pub data: Vec<u8>,
    /// Флаг: данные в `data` находятся в сжатом виде
    pub is_compressed: bool,
}
