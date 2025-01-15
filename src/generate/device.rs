use std::fmt::Write as _;

use anyhow::Result;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use crate::ir::*;
use crate::util::{self, StringExt};

use super::sorted;

pub fn render_device_x(_ir: &IR, d: &Device) -> Result<String> {
    let mut device_x = String::new();
    for i in sorted(&d.interrupts, |i| i.value) {
        writeln!(&mut device_x, "PROVIDE({} = DefaultHandler);", i.name).unwrap();
    }
    Ok(device_x)
}

pub fn render(opts: &super::Options, ir: &IR, d: &Device, path: &str) -> Result<TokenStream> {
    let mut out = TokenStream::new();
    let span = Span::call_site();

    let mut interrupts = TokenStream::new();
    let mut peripherals = TokenStream::new();
    let mut vectors = TokenStream::new();
    let mut names = vec![];
    let mut names_instances = vec![];
    let mut instances = TokenStream::new();
    let mut instance_fields = TokenStream::new();

    let mut pos = 0;
    for i in sorted(&d.interrupts, |i| i.value) {
        while pos < i.value {
            vectors.extend(quote!(Vector { _reserved: 0 },));
            pos += 1;
        }
        pos += 1;

        let name_uc = Ident::new(&i.name.to_sanitized_upper_case(), span);
        let description = format!(
            "{} - {}",
            i.value,
            i.description
                .as_ref()
                .map(|s| util::respace(s))
                .as_ref()
                .map(|s| util::escape_brackets(s))
                .unwrap_or_else(|| i.name.clone())
        );

        let value = util::unsuffixed(i.value as u64);

        interrupts.extend(quote! {
            #[doc = #description]
            #name_uc = #value,
        });
        vectors.extend(quote!(Vector { _handler: #name_uc },));
        names.push(name_uc);
    }

    for p in sorted(&d.peripherals, |p| p.base_address) {
        names_instances.push((
            Ident::new(&p.name.to_sanitized_pascal_case(), span),
            Ident::new(&p.name.to_sanitized_snake_case(), span),
        ));
        let name = Ident::new(&p.name.to_sanitized_pascal_case(), span);
        let address = util::hex_usize(p.base_address);
        let doc = util::doc(&p.description);

        if let Some(block_name) = &p.block {
            let _b = ir.blocks.get(block_name);
            let path = util::relative_path(block_name, path);

            peripherals.extend(quote! {
                #doc
                pub struct #name {
                    _p: (),
                }

                impl #name {
                    /// Conjure the register from thin air.
                    ///
                    /// Safety: It's up to the user to not alias memory or make data races.
                    pub const unsafe fn conjure() -> Self {
                        Self { _p: () }
                    }
                }

                impl core::ops::Deref for #name {
                    type Target = #path;

                    fn deref(&self) -> &Self::Target {
                        const INST: #path = unsafe { #path::from_ptr(#address as *mut ()) };

                        &INST
                    }
                }
            });
        } else {
            peripherals.extend(quote! {
                #doc
                pub const #name: *mut () = #address as _;
            });
        }
    }

    for (instance_pc, instance_sc) in names_instances {
        instance_fields.extend(quote!(
            pub #instance_sc: #instance_pc,
        ));
        instances.extend(quote!(
            #instance_sc: #instance_pc { _p: () },
        ));
    }

    let n = util::unsuffixed(pos as u64);

    let defmt = opts.defmt_feature.as_ref().map(|defmt_feature| {
        quote! {
            #[cfg_attr(feature = #defmt_feature, derive(defmt::Format))]
        }
    });

    out.extend(quote!(
        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        #defmt
        pub enum Interrupt {
            #interrupts
        }

        unsafe impl cortex_m::interrupt::InterruptNumber for Interrupt {
            #[inline(always)]
            fn number(self) -> u16 {
                self as u16
            }
        }

        #[cfg(feature = "rt")]
        mod _vectors {
            extern "C" {
                #(fn #names();)*
            }

            pub union Vector {
                _handler: unsafe extern "C" fn(),
                _reserved: u32,
            }

            #[link_section = ".vector_table.interrupts"]
            #[no_mangle]
            pub static __INTERRUPTS: [Vector; #n] = [
                #vectors
            ];
        }

        pub struct Instances {
            #instance_fields
        }

        impl Instances {
            /// Conjure all peripherals.
            ///
            /// Safety: Calling this more than once will alias the peripheral.
            pub const unsafe fn conjure() -> Self {
                Instances {
                    #instances
                }
            }
        }

        #peripherals
    ));

    if let Some(nvic_priority_bits) = d.nvic_priority_bits {
        let bits = util::unsuffixed(u64::from(nvic_priority_bits));
        out.extend(quote! {
            /// Number available in the NVIC for configuring priority
            #[cfg(feature = "rt")]
            pub const NVIC_PRIO_BITS: u8 = #bits;
        });
    }

    out.extend(quote! {
        #[cfg(feature = "rt")]
        pub use cortex_m_rt::interrupt;
        #[cfg(feature = "rt")]
        pub use Interrupt as interrupt;
    });

    Ok(out)
}
