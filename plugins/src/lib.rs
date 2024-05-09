use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::{collections::HashMap, fs};
use syn::{parse::Parse, parse_macro_input, token::Mut, Ident, PatIdent, Token};

struct MacroInput {
    list: Ident,
    comma: Token![,],
    plugin: Ident,
    comma2: Option<Token![,]>,
    mutability: Option<Mut>,
    plugin_type: Option<Ident>,
}

impl Parse for MacroInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            list: input.parse()?,
            comma: input.parse()?,
            plugin: input.parse()?,
            comma2: input.parse().ok(),
            mutability: input.parse().ok(),
            plugin_type: input.parse().ok(),
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

    let get_plugin = match input.plugin_type {
        Some(plugin_type) => {
            if plugin_type == "Option" {
                if input.mutability.is_some() {
                    quote!(match #list.get_mut(#index) {
                    Some(config) => config.as_mut().map(|plugin| {
                        unsafe { Arc::get_mut_unchecked(&mut plugin.config) }
                        .as_any_mut()
                        .downcast_mut::<#plugin_name::Config>()
                        .unwrap()}),
                    None => None,
                    })
                } else {
                    quote!(match #list.get(#index){
                        Some(config) => config.as_mut().map(|plugin| {
                        plugin.config
                            .as_any()
                            .downcast_ref::<#plugin_name::Config>()
                            .unwrap()}),
                        None => None,
                    })
                }
            } else {
                return quote! {compile_error!("invalid plugin name")}.into();
            }
        }
        None => {
            quote!(match #list.get(#index) {
                Some(config) => config.config.as_any().downcast_ref::<#plugin_name::Config>(),
                None => None,
            })
        }
    };

    get_plugin.into()
}

#[proc_macro]
pub fn load_plugins(_input: TokenStream) -> TokenStream {
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
        let plugin = jequi::load_plugin(config, &mut plugins).expect("main config is required");
        plugins.insert(0, Some(plugin));
        #(
        let plugin = #plugins::load_plugin(config, &mut plugins);
        plugins.insert(#indexes,plugin);
        )*
        plugins.into_iter().flatten().collect::<Vec<_>>()
        }
    })
    .into()
}
