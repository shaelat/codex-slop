#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HopView {
    pub ttl: u32,
    pub ip: Option<String>,
    pub loss: f64,
    pub median_rtt: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TargetView {
    pub name: String,
    pub hops: Vec<HopView>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub wave: u32,
    pub targets: Vec<TargetView>,
    pub last_detail: Option<String>,
}
