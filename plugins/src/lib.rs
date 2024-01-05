use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::{collections::HashMap, fs};
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
        let line = line.split('-').next().unwrap().trim();
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

    let mut add_after = HashMap::new();

    let mut plugins = Vec::new();
    let mut indexes = Vec::new();
    for (i, line) in contents.lines().enumerate() {
        let mut info = line.split('-');
        let plugin = info.next().unwrap().trim();
        if let Some(info) = info.next() {
            let requires = info.trim().strip_prefix("require ").unwrap().trim();
            add_after.insert(requires, (plugin, i + 1));
            continue;
        }
        plugins.push(format_ident!("{}", plugin));
        indexes.push(i + 1);
        if let Some((plugin, i)) = add_after.get(plugin) {
            plugins.push(format_ident!("{}", plugin));
            indexes.push(*i);
        }
    }

    let size = plugins.len() + 1;

    (quote! {
    pub fn load_plugins(config: &serde_yaml::Value) -> jequi::ConfigList {
        let mut plugins: Vec<Option<jequi::Plugin>> = Vec::with_capacity(#size);
        plugins.resize_with(#size, Default::default);
        plugins.insert(0,Some(jequi::load_plugin(config).expect("main config is required")));
        #(
        plugins.insert(#indexes,#plugins::load_plugin(config));
        )*
        plugins.into_iter().flatten().collect::<Vec<_>>()
        }
    })
    .into()
}
