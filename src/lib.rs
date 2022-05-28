#![doc = include_str!("../Readme.md")]
#![doc(issue_tracker_base_url = "https://github.com/recmo/uint/issues/")]
#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::nursery)]
#![cfg_attr(
    any(test, feature = "bench"),
    allow(clippy::wildcard_imports, clippy::cognitive_complexity)
)]
#![cfg_attr(
    all(has_generic_const_exprs, feature = "generic_const_exprs"),
    allow(incomplete_features)
)]
#![cfg_attr(
    all(has_generic_const_exprs, feature = "generic_const_exprs"),
    feature(generic_const_exprs)
)]

mod add;
mod algorithms;
pub mod aliases;
mod base_convert;
mod bit_vec;
mod bits;
mod bytes;
mod cmp;
mod const_for;
mod div;
mod from;
mod log;
mod mul;
mod pow;
mod special;
mod string;
mod support;
mod uint_dyn;
mod utils;

#[cfg(feature = "dyn")]
#[doc(inline)]
pub use uint_dyn::UintDyn;

#[doc(inline)]
pub use bit_vec::Bits;

#[doc(inline)]
pub use self::{base_convert::BaseConvertError, bytes::nbytes, string::ParseError};

#[doc(inline)]
pub use ruint_macro::uint;

// TODO: Have a `struct OverflowError` and use `Result<Self, OverflowError>`
// instead of `Option<Self>`.

#[cfg(all(has_generic_const_exprs, feature = "generic_const_exprs"))]
pub mod nightly {
    //! Extra features that are nightly only.

    /// Alias for `Uint` specified only by bit size.
    ///
    /// Compared to [`crate::Uint`] it compile-time computes the required number
    /// of limbs. Unfortunately this requires the nightly feature
    /// `generic_const_exprs`.
    ///
    /// # References
    /// * [Working group](https://rust-lang.github.io/project-const-generics/)
    ///   const generics working group.
    /// * [RFC2000](https://rust-lang.github.io/rfcs/2000-const-generics.html)
    ///   const generics.
    /// * [#60551](https://github.com/rust-lang/rust/issues/60551) associated
    ///   constants in const generics.
    /// * [#76560](https://github.com/rust-lang/rust/issues/76560) tracking
    ///   issue for `generic_const_exprs`.
    /// * [Rust blog](https://blog.rust-lang.org/inside-rust/2021/09/06/Splitting-const-generics.html)
    ///   2021-09-06 Splitting const generics.
    pub type Uint<const BITS: usize> = crate::Uint<BITS, { crate::nlimbs(BITS) }>;

    /// Alias for `Bits` specified only by bit size.
    ///
    /// See [`Uint`] for more information.
    pub type Bits<const BITS: usize> = crate::Bits<BITS, { crate::nlimbs(BITS) }>;
}

/// The ring of numbers modulo $2^{\mathtt{BITS}}$.
///
/// [`Uint`] implements nearly all traits and methods from the `std` unsigned
/// integer types, including most nightly only ones.
///
/// # Notable differences from `std` uint types.
///
/// * The operators `+`, `-`, `*`, etc. using wrapping math by default. The std
///   operators panic on overflow in debug, and are undefined in release, see
///   [reference][std-overflow].
/// * The [`Uint::checked_shl`], [`Uint::overflowing_shl`], etc return overflow
///   when non-zero bits are shifted out. In std they return overflow when the
///   shift amount is greater than the bit size.
/// * Some methods like [`u64::div_euclid`] and [`u64::rem_euclid`] are left out
///   because they are meaningless or redundant for unsigned integers. Std has
///   them for compatibility with their signed integers.
/// * Many functions that are `const` in std are not in [`Uint`].
/// * [`Uint::to_le_bytes`] and [`Uint::to_be_bytes`] require the output size to
///   be provided as a const-generic argument. They will runtime panic if the
///   provided size is incorrect.
///
/// [std-overflow]: https://doc.rust-lang.org/reference/expressions/operator-expr.html#overflow
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct Uint<const BITS: usize, const LIMBS: usize> {
    limbs: [u64; LIMBS],
}

impl<const BITS: usize, const LIMBS: usize> Uint<BITS, LIMBS> {
    /// The size of this integer type in 64-bit limbs.
    pub const LIMBS: usize = nlimbs(BITS);

    /// Bit mask for the last limb.
    const MASK: u64 = mask(BITS);

    /// The size of this integer type in bits.
    pub const BITS: usize = BITS;

    /// The smallest value that can be represented by this integer type.
    /// Synonym for [`Self::ZERO`].
    pub const MIN: Self = Self::ZERO;

    /// The value zero. This is the only value that exists in all [`Uint`]
    /// types.
    pub const ZERO: Self = Self { limbs: [0; LIMBS] };

    /// The largest value that can be represented by this integer type,
    /// $2^{\mathtt{BITS}} − 1$.
    pub const MAX: Self = {
        let mut limbs = [u64::MAX; LIMBS];
        if BITS > 0 {
            limbs[LIMBS - 1] &= Self::MASK;
        }
        Self { limbs }
    };

    /// View the array of limbs.
    #[must_use]
    pub const fn as_limbs(&self) -> &[u64; LIMBS] {
        &self.limbs
    }

    /// Access the array of limbs.
    // TODO: Can be made `const` with `#![feature(const_mut_refs)]`.
    #[must_use]
    pub fn as_limbs_mut(&mut self) -> &mut [u64; LIMBS] {
        &mut self.limbs
    }

    /// Convert to a array of limbs.
    ///
    /// Limbs are least significant first.
    #[must_use]
    pub const fn into_limbs(self) -> [u64; LIMBS] {
        self.limbs
    }

    /// # Panics
    ///
    /// Panics it `LIMBS` is not equal to `nlimbs(BITS)`.
    ///
    /// Panics if the value is to large for the bit-size of the Uint.
    #[must_use]
    #[track_caller]
    pub const fn from_limbs(limbs: [u64; LIMBS]) -> Self {
        Self::assert_valid();
        if BITS > 0 {
            // TODO: Add `<{BITS}>` to the type when Display works in const fn.
            assert!(
                limbs[Self::LIMBS - 1] <= Self::MASK,
                "Value too large for this Uint"
            );
        }
        Self { limbs }
    }

    #[must_use]
    #[track_caller]
    pub fn from_limbs_slice(slice: &[u64]) -> Self {
        let mut limbs = [0; LIMBS];
        limbs.copy_from_slice(slice);
        Self::from_limbs(limbs)
    }

    const fn assert_valid() {
        // TODO: Replace with `assert_eq!` when it is made `const`.
        // Blocked on Rust, not issue known.
        #[allow(clippy::manual_assert)]
        if LIMBS != Self::LIMBS {
            panic!("Can not construct Uint<BITS, LIMBS> with incorrect LIMBS");
        }
    }
}

impl<const BITS: usize, const LIMBS: usize> Default for Uint<BITS, LIMBS> {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Number of `u64` limbs required to represent the given number of bits.
/// This needs to be public because it is used in the `Uint` type.
#[must_use]
pub const fn nlimbs(bits: usize) -> usize {
    (bits + 63) / 64
}

/// Mask to apply to the highest limb to get the correct number of bits.
#[must_use]
const fn mask(bits: usize) -> u64 {
    if bits == 0 {
        return 0;
    }
    let bits = bits % 64;
    if bits == 0 {
        u64::MAX
    } else {
        (1 << bits) - 1
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_mask() {
        assert_eq!(mask(0), 0);
        assert_eq!(mask(1), 1);
        assert_eq!(mask(5), 0x1f);
        assert_eq!(mask(63), u64::max_value() >> 1);
        assert_eq!(mask(64), u64::max_value());
    }

    #[test]
    fn test_max() {
        assert_eq!(Uint::<0, 0>::MAX, Uint::ZERO);
        assert_eq!(Uint::<1, 1>::MAX, Uint::from_limbs([1]));
        assert_eq!(Uint::<7, 1>::MAX, Uint::from_limbs([127]));
        assert_eq!(Uint::<64, 1>::MAX, Uint::from_limbs([u64::MAX]));
        assert_eq!(
            Uint::<100, 2>::MAX,
            Uint::from_limbs([u64::MAX, u64::MAX >> 28])
        );
    }

    #[test]
    fn test_constants() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            assert_eq!(Uint::<BITS, LIMBS>::MIN, Uint::<BITS, LIMBS>::ZERO);
            let _ = Uint::<BITS, LIMBS>::MAX;
        });
    }
}

#[cfg(feature = "bench")]
pub mod bench {
    use super::*;
    use criterion::Criterion;

    pub fn group(criterion: &mut Criterion) {
        add::bench::group(criterion);
        mul::bench::group(criterion);
        div::bench::group(criterion);
        pow::bench::group(criterion);
        log::bench::group(criterion);
    }
}
