use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::fs;
use syn::{parse::Parse, parse_macro_input, Ident, Token};

struct MacroInput {
    list: Ident,
    comma: Token![,],
    plugin: Ident,
}

impl Parse for MacroInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            list: input.parse()?,
            comma: input.parse()?,
            plugin: input.parse()?,
        })
    }
}

#[proc_macro]
pub fn get_plugin(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as MacroInput);

    let plugin_name = input.plugin.to_string();

    let mut contents = match fs::read_to_string("plugins.txt") {
        Ok(content) => content,
        Err(err) => {
            let err = format!("error opening plugins file: {}", err);
            return quote! {compile_error!(#err)}.into();
        }
    };

    contents.insert_str(0, "jequi\n");

    let mut index = None;
    for (i, line) in contents.lines().enumerate() {
        if line == plugin_name {
            index = Some(i)
        }
    }

    let index = match index {
        Some(i) => i,
        None => return quote! {compile_error!("invalid plugin name")}.into(),
    };

    let list = input.list;
    let plugin_name = input.plugin;

    (quote! {
        #list[#index].config.as_any().downcast_ref::<#plugin_name::Config>().unwrap()
    })
    .into()
}

#[proc_macro]
pub fn load_plugins(input: TokenStream) -> TokenStream {
    let contents = match fs::read_to_string("plugins.txt") {
        Ok(content) => content,
        Err(err) => {
            let err = format!("error opening plugins file: {}", err);
            return quote! {compile_error!(#err)}.into();
        }
    };

    let mut plugins = Vec::new();
    for plugin in contents.lines() {
        plugins.push(format_ident!("{}", plugin))
    }

    (quote! {
    pub fn load_plugins(config: &serde_yaml::Value) -> jequi::ConfigList {
        let mut plugins: Vec<jequi::Plugin> = Vec::new();
        plugins.push(jequi::load_plugin(config).expect("main config is required"));
        #(
        if let Some(plugin) = #plugins::load_plugin(config) {
            plugins.push(plugin);
        }
        )*
        plugins
        }
    })
    .into()
}

// pub fn load_plugins(config: &Value) -> ConfigList {
//     // TODO: implement this function as a macro to load plugins dynamically
//     let mut plugins: Vec<Plugin> = Vec::new();
//     plugins.push(jequi::config::load_plugin(config).expect("main config is required"));
//     if let Some(plugin) = jequi_go::load_plugin(config) {
//         plugins.push(plugin);
//     }
//     if let Some(plugin) = jequi_serve_static::load_plugin(config) {
//         plugins.push(plugin);
//     }
//     if let Some(plugin) = jequi_proxy::load_plugin(config) {
//         plugins.push(plugin);
//     }
//     plugins
// }
