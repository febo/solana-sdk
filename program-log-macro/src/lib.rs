#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::arithmetic_side_effects)]

extern crate alloc;

use {
    alloc::{format, string::ToString, vec::Vec},
    proc_macro::TokenStream,
    proc_macro_crate::{crate_name, FoundCrate},
    quote::quote,
    regex::Regex,
    syn::{
        parse::{Parse, ParseStream},
        parse_macro_input, parse_str,
        punctuated::Punctuated,
        Error, Expr, ItemFn, LitInt, LitStr, Path, Token,
    },
};

/// The default buffer size for the logger.
const DEFAULT_BUFFER_SIZE: &str = "200";

/// The default name of the `solana-program-log` package to search for when
/// discovering the crate path.
const PROGRAM_LOG_PACKAGE_NAME: &str = "solana-program-log";

/// Represents the input arguments to the `log!` macro.
struct LogArgs {
    /// The path to the crate where the `Logger` struct is defined.
    crate_path: Path,

    /// The length of the buffer to use for the logger.
    ///
    /// This does not have effect when the literal `str` does
    /// not have value placeholders.
    buffer_len: LitInt,

    /// The literal formatting string passed to the macro.
    ///
    /// The `str` might have value placeholders. While this is
    /// not a requirement, the number of placeholders must
    /// match the number of args.
    format_string: LitStr,

    /// The arguments passed to the macro.
    ///
    /// The arguments represent the values to replace the
    /// placeholders on the format `str`. Valid values must implement
    /// the [`Log`] trait.
    args: Punctuated<Expr, Token![,]>,
}

impl Parse for LogArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let crate_path = input.parse::<Path>()?;
        // Parse the comma after the crat path.
        input.parse::<Token![,]>()?;

        // Optional buffer length.
        let buffer_len = if input.peek(LitInt) {
            let literal = input.parse()?;
            // Parse the comma after the buffer length.
            input.parse::<Token![,]>()?;
            literal
        } else {
            parse_str::<LitInt>(DEFAULT_BUFFER_SIZE)?
        };

        let format_string = input.parse()?;
        // Check if there are any arguments passed to the macro.
        let args = if input.is_empty() {
            Punctuated::new()
        } else {
            input.parse::<Token![,]>()?;
            Punctuated::parse_terminated(input)?
        };

        Ok(LogArgs {
            crate_path,
            buffer_len,
            format_string,
            args,
        })
    }
}

/// Represents the input arguments to the `log_cu_usage` attribute macro.
enum LogCuUsageArgs {
    /// Discover the crate path for the `Logger` struct by searching for
    /// the `solana-program-log` package in the dependency graph.
    Discover,

    /// Explicitly specify the crate path for the `Logger` struct.
    CratePath(Path),
}

impl Parse for LogCuUsageArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self::Discover);
        }

        // Support for `crate = <PATH>`.
        if input.peek(Token![crate]) {
            input.parse::<Token![crate]>()?;
            input.parse::<Token![=]>()?;

            let crate_path = input.parse::<Path>()?;

            if !input.is_empty() {
                return Err(input.error("unexpected tokens after `crate = ...`"));
            }

            return Ok(Self::CratePath(crate_path));
        }

        // Support for standalone path.
        let crate_path = input.parse::<Path>()?;

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after crate path"));
        }

        Ok(Self::CratePath(crate_path))
    }
}

fn discovered_log_crate() -> syn::Result<Path> {
    let crate_name = match crate_name(PROGRAM_LOG_PACKAGE_NAME) {
        Ok(FoundCrate::Itself) => "crate".to_string(),
        Ok(FoundCrate::Name(name)) => name,
        Err(error) => {
            return Err(Error::new(
                proc_macro::Span::call_site().into(),
                format!(
                    "failed to resolve `{PROGRAM_LOG_PACKAGE_NAME}` while expanding \
                     `solana-program-log-macro`: {error}"
                ),
            ));
        }
    };

    parse_str(&crate_name).map_err(|error| {
        Error::new(
            proc_macro::Span::call_site().into(),
            format!(
                "failed to parse `{crate_name}` as the crate path for `{PROGRAM_LOG_PACKAGE_NAME}`: {error}",
            ),
        )
    })
}

/// Companion `log!` macro.
///
/// The macro automates the creation of a `Logger` object to log a message.
/// It support a limited subset of the [`format!`](https://doc.rust-lang.org/std/fmt/) syntax.
/// The macro parses the format string at compile time and generates the calls to a `Logger`
/// object to generate the corresponding formatted message.
///
/// # Arguments
///
/// - `crate_path`: The path to the crate where the `Logger` struct is defined. This is a required argument.
/// - `buffer_len`: The length of the buffer to use for the logger (default to `200`). This is an optional argument.
/// - `format_string`: The literal string to log. This string can contain placeholders `{}` to be replaced by the arguments.
/// - `args`: The arguments to replace the placeholders in the format string. The arguments must implement the `Log` trait.
#[proc_macro]
pub fn log(input: TokenStream) -> TokenStream {
    // Parse the input into a `LogArgs`.
    let LogArgs {
        crate_path,
        buffer_len,
        format_string,
        args,
    } = parse_macro_input!(input as LogArgs);
    let parsed_string = format_string.value();

    // Regex pattern to match placeholders in the format string.
    let placeholder_regex = Regex::new(r"\{.*?\}").unwrap();

    let placeholders: Vec<_> = placeholder_regex
        .find_iter(&parsed_string)
        .map(|m| m.as_str())
        .collect();

    // Check if there is an argument for each `{}` placeholder.
    if placeholders.len() != args.len() {
        let arg_message = if args.is_empty() {
            "but no arguments were given".to_string()
        } else {
            format!(
                "but there is {} {}",
                args.len(),
                if args.len() == 1 {
                    "argument"
                } else {
                    "arguments"
                }
            )
        };

        return Error::new_spanned(
            format_string,
            format!(
                "{} positional arguments in format string, {}",
                placeholders.len(),
                arg_message
            ),
        )
        .to_compile_error()
        .into();
    }

    if !placeholders.is_empty() {
        // The parts of the format string with the placeholders replaced by arguments.
        let mut replaced_parts = Vec::new();

        let parts: Vec<&str> = placeholder_regex.split(&parsed_string).collect();
        let part_iter = parts.iter();

        let mut arg_iter = args.iter();
        let mut ph_iter = placeholders.iter();

        // Replace each occurrence of `{}` with their corresponding argument value.
        for part in part_iter {
            if !part.is_empty() {
                replaced_parts.push(quote! { logger.append(#part) });
            }

            if let Some(arg) = arg_iter.next() {
                // The number of placeholders was validated to be the same as
                // the number of arguments, so this should never panic.
                let placeholder = ph_iter.next().unwrap();

                match *placeholder {
                    "{}" => {
                        replaced_parts.push(quote! { logger.append(#arg) });
                    }
                    value if value.starts_with("{:.") => {
                        let Ok(precision) = value[3..value.len() - 1].parse::<u8>() else {
                            return Error::new_spanned(
                                format_string,
                                format!("invalid precision format: {value}"),
                            )
                            .to_compile_error()
                            .into();
                        };

                        replaced_parts.push(quote! {
                            logger.append_with_args(
                                #arg,
                                &[#crate_path::logger::Argument::Precision(#precision)]
                            )
                        });
                    }
                    value if value.starts_with("{:<.") || value.starts_with("{:>.") => {
                        let Ok(size) = value[4..value.len() - 1].parse::<usize>() else {
                            return Error::new_spanned(
                                format_string,
                                format!("invalid truncate size format: {value}"),
                            )
                            .to_compile_error()
                            .into();
                        };

                        match value.chars().nth(2) {
                            Some('<') => {
                                replaced_parts.push(quote! {
                                    logger.append_with_args(
                                        #arg,
                                        &[#crate_path::logger::Argument::TruncateStart(#size)]
                                    )
                                });
                            }
                            Some('>') => {
                                replaced_parts.push(quote! {
                                    logger.append_with_args(
                                        #arg,
                                        &[#crate_path::logger::Argument::TruncateEnd(#size)]
                                    )
                                });
                            }
                            _ => {
                                // This should not happen since we already checked the format.
                                return Error::new_spanned(
                                    format_string,
                                    format!("invalid truncate format: {value}"),
                                )
                                .to_compile_error()
                                .into();
                            }
                        }
                    }
                    _ => {
                        return Error::new_spanned(
                            format_string,
                            format!("invalid placeholder: {placeholder}"),
                        )
                        .to_compile_error()
                        .into();
                    }
                }
            }
        }

        // Generate the output string as a compile-time constant
        TokenStream::from(quote! {
            {
                let mut logger = #crate_path::logger::Logger::<#buffer_len>::default();
                #(#replaced_parts;)*
                logger.log();
            }
        })
    } else {
        TokenStream::from(quote! {
            {
                #crate_path::logger::log_message(#format_string.as_bytes());
            }
        })
    }
}

/// Attribute macro for instrumenting functions with compute unit logging.
///
/// This macro wraps the decorated function with additional logging statements
/// that print the function name and the number of compute units used before and after
/// the function execution.
///
/// # Effects
///
/// - Adds a log message with the function name at the end of execution with amount of CU consumed.
///
/// # Note
///
/// This macro consumes an additional compute units per call due to the logging operations.
///
///  # Example
///
/// ```rust,ignore
/// #[solana_program_log::log_cu_usage]
/// fn my_function() {
///     // Function body
/// }
/// ```
///
/// logging output will look like:
///
/// "Program log: Function `my_function` consumed 36 compute units"
///
/// # References
///
/// * [Logging syscall](https://github.com/anza-xyz/agave/blob/d88050cda335f87e872eddbdf8506bc063f039d3/programs/bpf_loader/src/syscalls/logging.rs#L70)
/// * [Compute budget](https://github.com/anza-xyz/agave/blob/d88050cda335f87e872eddbdf8506bc063f039d3/program-runtime/src/compute_budget.rs#L150)
///
#[proc_macro_attribute]
pub fn log_cu_usage(attr: TokenStream, item: TokenStream) -> TokenStream {
    let crate_path = match parse_macro_input!(attr as LogCuUsageArgs) {
        LogCuUsageArgs::Discover => match discovered_log_crate() {
            Ok(path) => path,
            Err(error) => return error.to_compile_error().into(),
        },
        LogCuUsageArgs::CratePath(path) => path,
    };
    let mut input = parse_macro_input!(item as ItemFn);
    let fn_name = &input.sig.ident;
    let block = &input.block;

    input.block = syn::parse_quote!({
        let cu_before = unsafe { #crate_path::logger::remaining_compute_units() };

        let __result = (|| #block)();

        let cu_after = unsafe { #crate_path::logger::remaining_compute_units() };
        // 100 (compute budget syscall_base_cost) + 2 (extra calculations)
        let introspection_cost = 102;

        let consumed = cu_before - cu_after - introspection_cost;

        #crate_path::log!("Function {} consumed {} compute units", stringify!(#fn_name), consumed);

        __result
    });

    quote!(#input).into()
}

#[cfg(test)]
mod tests {
    use {
        super::LogCuUsageArgs,
        syn::{parse_quote, parse_str, Path},
    };

    #[test]
    fn log_cu_usage_args_support_empty_input() {
        assert!(matches!(
            parse_str::<LogCuUsageArgs>("").unwrap(),
            LogCuUsageArgs::Discover
        ));
    }

    #[test]
    fn log_cu_usage_args_support_standalone_path() {
        let args = parse_str::<LogCuUsageArgs>("mylog").unwrap();
        match args {
            LogCuUsageArgs::CratePath(path) => assert_eq!(path, parse_quote!(mylog)),
            LogCuUsageArgs::Discover => panic!("expected explicit crate path"),
        }
    }

    #[test]
    fn log_cu_usage_args_support_crate_equals_path() {
        let args = parse_str::<LogCuUsageArgs>("crate = another_log").unwrap();
        match args {
            LogCuUsageArgs::CratePath(path) => {
                let expected: Path = parse_quote!(another_log);
                assert_eq!(path, expected);
            }
            LogCuUsageArgs::Discover => panic!("expected explicit crate path"),
        }
    }
}
