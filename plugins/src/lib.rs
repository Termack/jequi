use jequi::Plugin;
use serde_yaml::Value;

pub fn load_plugins(config: &Value) -> Vec<Plugin> {
    // TODO: implement this function as a macro to load plugins dynamically
    let mut plugins: Vec<Plugin> = Vec::new();
    plugins.push(jequi::config::load_plugin(config).expect("main config is required"));
    if let Some(plugin) = jequi_go::load_plugin(config) {
        plugins.push(plugin);
    }
    if let Some(plugin) = jequi_serve_static::load_plugin(config) {
        plugins.push(plugin);
    }
    if let Some(plugin) = jequi_proxy::load_plugin(config) {
        plugins.push(plugin);
    }
    plugins
}
