// SVG loading for `Image` & `loadImage()`, with <style> rules resolved to inline styles (since Skia's SVG DOM
// ignores them)
//
// stylesheet.rs, selector.rs, and stream.rs are adapted from the `simplecss` crate (v0.2.2,
// https://github.com/linebender/simplecss) and extended with structural pseudo-classes, `:not()`, the `~`
// combinator, custom properties & `var()`, and declaration-level error recovery.
// Tests: `cargo test --lib svg::tests` (not part of `make test`)
//
// Those three files were originally dual-licensed as `Apache-2.0 OR MIT`; they're used here under the MIT license:
//
// The MIT License (MIT)
//
// Copyright (c) 2018 Reizner Evgeniy
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

mod document;
mod stylesheet;
mod selector;
mod stream;
#[cfg(test)]
mod tests;

pub use document::read_document;
