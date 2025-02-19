//! Module [`validator_entrypoint`](crate::validator_entrypoint) macro implementation

use super::*;

/// [`validator_entrypoint`](crate::validator_entrypoint()) macro implementation
#[allow(clippy::needless_pass_by_value)]
pub fn impl_entrypoint(attr: TokenStream, item: TokenStream) -> TokenStream {
    let fn_item = parse_macro_input!(item as syn::ItemFn);

    assert!(
        attr.is_empty(),
        "`#[entrypoint]` macro for Validator entrypoints accepts no attributes"
    );

    macro_rules! match_entrypoints {
        (validate: {
            $($user_entrypoint_name:ident =>
                $generated_entrypoint_name:ident ($query_validating_object_fn_name:ident)),* $(,)?
        }
        other: {
            $($other_user_entrypoint_name:ident => $branch:block),* $(,)?
        }) => {
            match &fn_item.sig.ident {
                $(fn_name if fn_name == stringify!($user_entrypoint_name) => {
                    impl_validate_entrypoint(
                        fn_item,
                        stringify!($user_entrypoint_name),
                        stringify!($generated_entrypoint_name),
                        stringify!($query_validating_object_fn_name),
                    )
                })*
                $(fn_name if fn_name == stringify!($other_user_entrypoint_name) => $branch),*
                _ => panic!(
                    "Validator entrypoint name must be one of: {:?}",
                    [
                        $(stringify!($user_entrypoint_name),)*
                        $(stringify!($other_user_entrypoint_name),)*
                    ]
                ),
            }
        };
    }

    match_entrypoints! {
        validate: {
            validate_transaction => _iroha_validator_validate_transaction(get_transaction_to_validate),
            validate_instruction => _iroha_validator_validate_instruction(get_instruction_to_validate),
            validate_query => _iroha_validator_validate_query(get_query_to_validate),
        }
        other: {
            migrate => { impl_migrate_entrypoint(fn_item) }
        }
    }
}

fn impl_validate_entrypoint(
    fn_item: syn::ItemFn,
    user_entrypoint_name: &'static str,
    generated_entrypoint_name: &'static str,
    query_validating_object_fn_name: &'static str,
) -> TokenStream {
    let syn::ItemFn {
        attrs,
        vis,
        sig,
        mut block,
    } = fn_item;
    let fn_name = &sig.ident;

    assert!(
        matches!(sig.output, syn::ReturnType::Type(_, _)),
        "Validator `{user_entrypoint_name}` entrypoint must have `Result` return type"
    );

    block.stmts.insert(
        0,
        parse_quote!(
            use ::iroha_validator::iroha_wasm::{ExecuteOnHost as _, QueryHost as _};
        ),
    );

    let generated_entrypoint_ident: syn::Ident = syn::parse_str(generated_entrypoint_name)
        .expect("Provided entrypoint name to generate is not a valid Ident, this is a bug");

    let query_validating_object_fn_ident: syn::Ident =
        syn::parse_str(query_validating_object_fn_name).expect(
            "Provided function name to query validating object is not a valid Ident, this is a bug",
        );

    let args = quote! {
        ::iroha_validator::iroha_wasm::get_authority(),
        ::iroha_validator::iroha_wasm::#query_validating_object_fn_ident(),
        ::iroha_validator::iroha_wasm::get_block_height(),
    };

    quote! {
        /// Validator `validate` entrypoint
        ///
        /// # Memory safety
        ///
        /// This function transfers the ownership of allocated
        /// [`Result`](::iroha_validator::iroha_wasm::data_model::validator::Result)
        #[no_mangle]
        #[doc(hidden)]
        unsafe extern "C" fn #generated_entrypoint_ident() -> *const u8 {
            let verdict: ::iroha_validator::iroha_wasm::data_model::validator::Result = #fn_name(#args);
            let bytes_box = ::core::mem::ManuallyDrop::new(::iroha_validator::iroha_wasm::encode_with_length_prefix(&verdict));

            bytes_box.as_ptr()
        }

        // NOTE: Host objects are always passed by value to wasm
        #[allow(clippy::needless_pass_by_value)]
        #(#attrs)*
        #vis #sig
        #block
    }
    .into()
}

fn impl_migrate_entrypoint(fn_item: syn::ItemFn) -> TokenStream {
    let syn::ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = fn_item;
    let fn_name = &sig.ident;

    assert!(
        matches!(sig.output, syn::ReturnType::Type(_, _)),
        "Validator `migrate()` entrypoint must have `MigrationResult` return type"
    );

    let args = quote! {
        ::iroha_validator::iroha_wasm::get_block_height(),
    };

    quote! {
        /// Validator `permission_token_schema` entrypoint
        ///
        /// # Memory safety
        ///
        /// This function transfers the ownership of allocated [`Vec`](alloc::vec::Vec).
        #[no_mangle]
        #[doc(hidden)]
        unsafe extern "C" fn _iroha_validator_migrate() -> *const u8 {
            let res: ::iroha_validator::data_model::validator::MigrationResult = #fn_name(#args);
            let bytes = ::core::mem::ManuallyDrop::new(::iroha_validator::iroha_wasm::encode_with_length_prefix(&res));

            ::core::mem::ManuallyDrop::new(bytes).as_ptr()
        }

        // NOTE: Host objects are always passed by value to wasm
        #[allow(clippy::needless_pass_by_value)]
        #(#attrs)*
        #vis #sig
        #block
    }
    .into()
}
