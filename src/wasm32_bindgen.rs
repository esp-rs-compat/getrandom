// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation for WASM via wasm-bindgen

use std::cell::RefCell;
use std::mem;

use wasm_bindgen::prelude::*;

use super::__wbg_shims::*;
use super::{Error, UNAVAILABLE_ERROR};
use super::utils::use_init;


#[derive(Clone, Debug)]
pub enum RngSource {
    Node(NodeCrypto),
    Browser(BrowserCrypto),
}

thread_local!(
    static RNG_SOURCE: RefCell<Option<RngSource>> = RefCell::new(None);
);

pub fn getrandom_inner(dest: &mut [u8]) -> Result<(), Error> {
    assert_eq!(mem::size_of::<usize>(), 4);

    RNG_SOURCE.with(|f| {
        use_init(f, getrandom_init, |source| {
            match *source {
                RngSource::Node(ref n) => n.random_fill_sync(dest),
                RngSource::Browser(ref n) => {
                    // see https://developer.mozilla.org/en-US/docs/Web/API/Crypto/getRandomValues
                    //
                    // where it says:
                    //
                    // > A QuotaExceededError DOMException is thrown if the
                    // > requested length is greater than 65536 bytes.
                    for chunk in dest.chunks_mut(65536) {
                        n.get_random_values(chunk)
                    }
                }
            }
            Ok(())
        })
    })

}

fn getrandom_init() -> Result<RngSource, Error> {
    // First up we need to detect if we're running in node.js or a
    // browser. To do this we get ahold of the `this` object (in a bit
    // of a roundabout fashion).
    //
    // Once we have `this` we look at its `self` property, which is
    // only defined on the web (either a main window or web worker).
    let this = Function::new("return this").call(&JsValue::undefined());
    assert!(this != JsValue::undefined());
    let this = This::from(this);
    let is_browser = this.self_() != JsValue::undefined();

    if !is_browser {
        return Ok(RngSource::Node(node_require("crypto")))
    }

    // If `self` is defined then we're in a browser somehow (main window
    // or web worker). Here we want to try to use
    // `crypto.getRandomValues`, but if `crypto` isn't defined we assume
    // we're in an older web browser and the OS RNG isn't available.
    let crypto = this.crypto();
    if crypto.is_undefined() {
        let msg = "self.crypto is undefined";
        return Err(UNAVAILABLE_ERROR)   // TODO: report msg
    }

    // Test if `crypto.getRandomValues` is undefined as well
    let crypto: BrowserCrypto = crypto.into();
    if crypto.get_random_values_fn().is_undefined() {
        let msg = "crypto.getRandomValues is undefined";
        return Err(UNAVAILABLE_ERROR)   // TODO: report msg
    }

    // Ok! `self.crypto.getRandomValues` is a defined value, so let's
    // assume we can do browser crypto.
    Ok(RngSource::Browser(crypto))
}
