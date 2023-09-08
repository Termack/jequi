use std::sync::Arc;

use jequi::{ConfigMap, JequiConfig, RequestHandler};

pub fn load_configs(
    mut config: ConfigMap,
) -> (Vec<Arc<dyn JequiConfig>>, Vec<Arc<dyn RequestHandler>>) {
    // TODO: implement this function as a macro to load plugins dynamically
    let mut handlers: Vec<Arc<dyn RequestHandler>> = Vec::new();
    let mut plugins: Vec<Arc<dyn JequiConfig>> = Vec::new();
    plugins.push(jequi::Config::load(&mut config, &mut handlers).expect("main config is required"));
    if let Some(plugin) = jequi_go::Config::load(&mut config, &mut handlers) {
        plugins.push(plugin);
    }
    if let Some(plugin) = jequi_serve_static::Config::load(&mut config, &mut handlers) {
        plugins.push(plugin);
    }
    (plugins, handlers)
}
