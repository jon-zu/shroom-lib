use darling::{
    ast::{self, Data, GenericParamExt},
    util, FromDeriveInput, FromField, FromMeta, ToTokens,
};
use proc_macro2::{Span, TokenStream};
use syn::{
    parse_quote, GenericParam, Generics, Ident, Lifetime, LifetimeParam, Type, TypeParamBound,
};

mod enum_impl;

/// Conditional Meta data, the field to check and the 'cond'ition function to call
#[derive(FromMeta, Debug)]
struct Cond {
    pub field: syn::Ident,
    pub cond: syn::Path,
}

impl Cond {
    /// Expr to access the field via self
    pub fn self_expr(&self) -> TokenStream {
        let cond_fn = &self.cond;
        let field = &self.field;
        quote::quote! ( #cond_fn( &self.#field ) )
    }

    /// Expr to access the field directely
    pub fn id_expr(&self) -> TokenStream {
        let cond_fn = &self.cond;
        let field = &self.field;
        quote::quote! ( #cond_fn( &#field ) )
    }
}

/// A field of the packet
#[derive(Debug, FromField)]
#[darling(attributes(pkt))]
struct PacketField {
    // Ident can be optional for unnamed structs
    ident: Option<Ident>,
    // Type
    ty: Type,
    // Check conditional
    check: Option<Cond>,
    // Either conditional
    either: Option<Cond>,
    // Size for `DecodePacketSized` + `EncodePacketSized`
    size: Option<Ident>,
}

impl PacketField {
    /// Get condition field to check
    pub fn get_cond(&self) -> Option<&Cond> {
        self.check.as_ref().or(self.either.as_ref())
    }

    /// Get the encode_len expr for this field
    pub fn encode_len_expr(&self, field_name: &TokenStream) -> TokenStream {
        if let Some(cond) = self.get_cond() {
            let cond = cond.self_expr();
            quote::quote! ( shroom_pkt::PacketConditional::encode_len_cond(&self.#field_name, #cond) )
        } else {
            quote::quote! ( self.#field_name.encode_len() )
        }
    }

    /// Get the size_hint expr for this field
    pub fn size_hint_expr(&self) -> TokenStream {
        let ty = &self.ty;
        // Conditional has no SizeHint
        if self.get_cond().is_some() {
            quote::quote!(shroom_pkt::SizeHint::NONE)
        } else {
            quote::quote!( <#ty>::SIZE_HINT )
        }
    }

    /// Get the encode expression for this field
    pub fn encode_expr(&self, field_name: &TokenStream) -> TokenStream {
        if let Some(cond) = self.get_cond() {
            let cond = cond.self_expr();
            quote::quote! ( shroom_pkt::PacketConditional::encode_cond(&self.#field_name, #cond, pw) )
        } else {
            quote::quote! ( self.#field_name.encode(pw) )
        }
    }

    /// Get the decode expr for this field
    pub fn decode_expr(&self, var_ident: &Ident) -> TokenStream {
        let ty = &self.ty;
        // Generate the condition check and call the decoder
        if let Some(cond) = self.get_cond() {
            let cond = cond.id_expr();
            quote::quote!( let #var_ident  = <#ty as shroom_pkt::PacketConditional>::decode_cond(#cond, pr) )

            // Call the sized decoder with the given sized expression
        } else if let Some(sz) = self.size.as_ref() {
            quote::quote!( let #var_ident = shroom_pkt::DecodePacketSized::decode_sized(pr, #sz as usize) )
        } else {
            quote::quote!( let #var_ident = <#ty>::decode(pr) )
        }
    }
}

/// Represent a packet with all fields
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(pkt), supports(struct_any))]
struct ShroomPacket {
    ident: Ident,
    data: ast::Data<util::Ignored, PacketField>,
    generics: syn::Generics,
}

impl ShroomPacket {
    /// Return all field with their actual names
    /// For named structs that's the actual name
    /// For unnamed structs that's the zero based index, prefixed by _ to get a valid ident
    fn fields_with_name(&self) -> impl Iterator<Item = ((Ident, TokenStream), &PacketField)> {
        let Data::Struct(ref fields) = self.data else {
            panic!("Not a struct");
        };

        fields.iter().enumerate().map(|(i, field)| {
            let ident = field
                .ident
                .as_ref()
                .map(|v| (v.clone(), quote::quote!(#v)))
                .unwrap_or_else(|| {
                    let i = syn::Index::from(i);
                    (quote::format_ident!("_{}", i), quote::quote!(#i))
                });

            (ident, field)
        })
    }

    /// Generate decode expr
    fn gen_decode(&self, token_stream: &mut proc_macro2::TokenStream) -> syn::Result<()> {
        let struct_name = &self.ident;

        let mut dec_generics = self.generics.clone();

        // Return a deserialize lifetime
        let de_lifetime = find_or_add_de_lifetime(&mut dec_generics).clone();
        // Add lifetime as bound to each existing bound
        let dec_generics = add_trait_bounds(
            dec_generics,
            parse_quote!(shroom_pkt::DecodePacket<#de_lifetime>),
        );

        // Get type generics
        let (_, ty_generics, _) = self.generics.split_for_impl();
        let (de_impl_generics, _, de_where_clause) = dec_generics.split_for_impl();

        // Generate the sequence of `let x = decode` decodings
        // this is required so the conditional checks are working
        let dec_var = self.fields_with_name().map(|((var_ident, _), field)| {
            let dec = field.decode_expr(&var_ident);
            quote::quote!( #dec?; )
        });

        // Set the actual fields
        let struct_dec_fields = self.fields_with_name().map(|((var_ident, field_name), _)| {
            quote::quote! { #field_name: #var_ident, }
        });

        token_stream.extend(quote::quote!(impl #de_impl_generics  shroom_pkt::DecodePacket<#de_lifetime> for #struct_name #ty_generics #de_where_clause  {
            fn decode(pr: &mut shroom_pkt::PacketReader<#de_lifetime>) -> shroom_pkt::PacketResult<Self> {
                #(#dec_var)*
                Ok(#struct_name {
                    #(#struct_dec_fields)*
                })
            }
        }));
        Ok(())
    }

    /// Generate encode expr
    fn gen_encode(&self, token_stream: &mut proc_macro2::TokenStream) -> syn::Result<()> {
        let struct_name = &self.ident;
        let enc_generics = add_trait_bounds(
            self.generics.clone(),
            parse_quote!(shroom_pkt::EncodePacket),
        );

        let (impl_generics, ty_generics, where_clause) = enc_generics.split_for_impl();

        // Generate the sequence of encodes for each fields
        let struct_enc_fields = self.fields_with_name().map(|((_, field_name), field)| {
            let enc = field.encode_expr(&field_name);
            quote::quote!( #enc?; )
        });

        // Generate the sequence of const SizeHints for each field and concat them with .add()
        let struct_size_hint_fields = self.fields_with_name().map(|(_, field)| {
            let hint = field.size_hint_expr();
            quote::quote!(.add(#hint))
        });

        // Generate the sequence of the encode_len determined at runtime
        let struct_encode_len_fields = self.fields_with_name().map(|((_, field_name), field)| {
            let len = field.encode_len_expr(&field_name);
            quote::quote!( + #len )
        });

        // Generate EncodePacket
        token_stream.extend(quote::quote!(impl #impl_generics shroom_pkt::EncodePacket for #struct_name #ty_generics #where_clause {
            fn encode<B: bytes::BufMut>(&self, pw: &mut shroom_pkt::PacketWriter<B>) -> shroom_pkt::PacketResult<()> {
                #(#struct_enc_fields)*
                Ok(())
            }

            const SIZE_HINT: shroom_pkt::SizeHint = shroom_pkt::SizeHint::ZERO #(#struct_size_hint_fields)*;

            fn encode_len(&self) -> usize {
                0 #(#struct_encode_len_fields)*
            }
        }));
        Ok(())
    }

    /// Generate encode and decode expr
    fn gen(&self, tokens: &mut proc_macro2::TokenStream) {
        self.gen_encode(tokens)
            .and_then(|_| self.gen_decode(tokens))
            .unwrap();
    }

    fn gen_encode_len(&self, tokens: &mut proc_macro2::TokenStream) {
        self.gen_encode(tokens).unwrap();
    }
}

/// EncodePacket is essentially a wrapper around ShroomPacket, which just generates the Encode part
struct EncodePacket(ShroomPacket);

impl ToTokens for EncodePacket {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.gen_encode_len(tokens);
    }
}

impl ToTokens for ShroomPacket {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.gen(tokens);
    }
}

/// Add the given trait bound to each generic parameter
fn add_trait_bounds(mut generics: Generics, bound: TypeParamBound) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(bound.clone());
        }
    }
    generics
}

fn find_or_add_de_lifetime(generics: &mut Generics) -> &Lifetime {
    // Find first lifetime and use that a de-serialization lifetime
    let first_lifetime = generics
        .params
        .iter()
        .position(|param| matches!(param, GenericParam::Lifetime(_)));

    // If lifetime is found use that
    &match first_lifetime {
        Some(ix) => &generics.params[ix],
        // Else insert new lifetime with the name 'de
        None => {
            let lf = Lifetime::new("'de", Span::call_site());
            let ty_lf: GenericParam = LifetimeParam::new(lf).into();
            generics.params.push(ty_lf);
            generics.params.last().expect("Last param must exist")
        }
    }
    .as_lifetime_param()
    .expect("must be Lifetime")
    .lifetime
}

#[proc_macro_derive(ShroomPacket, attributes(pkt))]
pub fn shroom_net(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let derive_input = syn::parse_macro_input!(item as syn::DeriveInput);

    let input = match ShroomPacket::from_derive_input(&derive_input) {
        Ok(input) => input,
        Err(err) => return err.write_errors().into(),
    };

    input.to_token_stream().into()
}

#[proc_macro_derive(ShroomEncodePacket, attributes(pkt))]
pub fn encode(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let derive_input = syn::parse_macro_input!(item as syn::DeriveInput);

    let input = match ShroomPacket::from_derive_input(&derive_input) {
        Ok(input) => input,
        Err(err) => return err.write_errors().into(),
    };

    EncodePacket(input).to_token_stream().into()
}

#[proc_macro_derive(ShroomPacketEnum, attributes(pkt))]
pub fn shroom_enum(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let derive_input = syn::parse_macro_input!(item as syn::DeriveInput);

    let input = match enum_impl::ShroomPacketEnum::from_derive_input(&derive_input) {
        Ok(input) => input,
        Err(err) => return err.write_errors().into(),
    };

    input.to_token_stream().into()
}
