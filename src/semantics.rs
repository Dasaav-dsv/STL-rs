//! Set of traits describing C++ move and copy semantics.
//!
//! They provide the "glue" for CSTL in the form of drop, copy and move function tables.

use std::{
    mem::{self, MaybeUninit},
    ptr::{self, NonNull},
};

use cstl_sys::{CSTL_CopyType, CSTL_DropType, CSTL_MoveType, CSTL_Type};

/// Trait for sized types.
///
/// Besides the size and alignment, also provides a [`CSTL_DropType`] table.
pub trait BaseType: Sized {
    /// CSTL type handle.
    const TYPE: CSTL_Type = if Self::SIZE & Self::ALIGN == 0 {
        usize::wrapping_neg(Self::SIZE | Self::ALIGN)
    } else {
        Self::SIZE
    } as CSTL_Type;

    /// Size of type.
    ///
    /// For ZSTs it is equal to 1.
    const SIZE: usize = if mem::size_of::<Self>() != 0 {
        mem::size_of::<Self>()
    } else {
        1
    };

    /// Alignment of type.
    const ALIGN: usize = mem::align_of::<Self>();

    /// CSTL destructible type table.
    const DROP: CSTL_DropType = CSTL_DropType {
        drop: unsafe { Some(mem::transmute(Self::raw_drop as *const ())) },
    };

    unsafe extern "C" fn raw_drop(first: NonNull<Self>, last: NonNull<Self>) {
        unsafe {
            let len = last
                .offset_from(first)
                .try_into()
                .expect("`first` > `last`");

            ptr::slice_from_raw_parts_mut(first.as_ptr(), len).drop_in_place();
        }
    }
}

impl<T> BaseType for T {}

/// Trait for types that can be moved with C++ semantics.
///
/// A C++ move is not destructive, so it has to leave an initialized value
/// in its place, which is why this trait requires [`Default`].
///
/// Provides a [`CSTL_MoveType`] table.
pub trait MoveType: Default + Sized {
    /// CSTL movable type table.
    const MOVE: CSTL_MoveType = CSTL_MoveType {
        drop_type: <Self as BaseType>::DROP,
        move_: unsafe { Some(mem::transmute(Self::raw_move as *const ())) },
    };

    unsafe extern "C" fn raw_move(first: NonNull<Self>, last: NonNull<Self>, dest: NonNull<Self>) {
        unsafe {
            for i in 0..last.offset_from(first) {
                dest.offset(i).write(mem::take(first.offset(i).as_mut()));
            }
        }
    }
}

impl<T: Default> MoveType for T {}

/// Trait for types that can be copied and moved with C++ semantics.
///
/// Requires [`Clone`], but also [`Default`], see [`MoveType`] for the justification.
///
/// Provides a [`CSTL_CopyType`] table.
pub trait CopyMoveType: Clone + Default + Sized {
    /// CSTL copyable type table.
    const COPY: CSTL_CopyType = CSTL_CopyType {
        move_type: <Self as MoveType>::MOVE,
        copy: unsafe { Some(mem::transmute(Self::raw_copy as *const ())) },
        fill: unsafe { Some(mem::transmute(Self::raw_fill as *const ())) },
    };

    unsafe extern "C" fn raw_copy(first: NonNull<Self>, last: NonNull<Self>, dest: NonNull<Self>) {
        unsafe {
            for i in 0..last.offset_from(first) {
                dest.offset(i).write(first.offset(i).as_ref().clone());
            }
        }
    }

    unsafe extern "C" fn raw_fill(first: NonNull<Self>, last: NonNull<Self>, value: NonNull<Self>) {
        unsafe {
            for i in 0..last.offset_from(first) {
                first.offset(i).write(value.as_ref().clone());
            }
        }
    }
}

impl<T: Clone + Default> CopyMoveType for T {}

/// Trait for types that can be copied and moved with C++ semantics.
///
/// Unlike [`CopyMoveType`] it is for types that are [`Clone`] but aren't [`Default`].
///
/// C++ move semantics imply that a copy is a valid kind of move,
/// as it leaves the destination equal to the source as it was before,
/// and leaves the source initialized.
///
/// Provides a [`CSTL_CopyType`] table.
pub trait CopyOnlyType: Clone + Sized {
    /// CSTL copyable type table.
    const COPY: CSTL_CopyType = CSTL_CopyType {
        move_type: CSTL_MoveType {
            drop_type: <Self as BaseType>::DROP,
            move_: unsafe { Some(mem::transmute(Self::raw_move as *const ())) },
        },
        copy: unsafe { Some(mem::transmute(Self::raw_copy as *const ())) },
        fill: unsafe { Some(mem::transmute(Self::raw_fill as *const ())) },
    };

    unsafe extern "C" fn raw_move(first: NonNull<Self>, last: NonNull<Self>, dest: NonNull<Self>) {
        unsafe {
            for i in 0..last.offset_from(first) {
                dest.offset(i).write(first.offset(i).as_ref().clone());
            }
        }
    }

    unsafe extern "C" fn raw_copy(first: NonNull<Self>, last: NonNull<Self>, dest: NonNull<Self>) {
        unsafe {
            for i in 0..last.offset_from(first) {
                dest.offset(i).write(first.offset(i).as_ref().clone());
            }
        }
    }

    unsafe extern "C" fn raw_fill(first: NonNull<Self>, last: NonNull<Self>, value: NonNull<Self>) {
        unsafe {
            for i in 0..last.offset_from(first) {
                first.offset(i).write(value.as_ref().clone());
            }
        }
    }
}

impl<T: Clone> CopyOnlyType for T {}

pub(crate) struct DefaultUninit<T>(MaybeUninit<T>);

impl<T> DefaultUninit<T> {
    pub const fn new(val: T) -> Self {
        Self(MaybeUninit::new(val))
    }

    pub const fn as_mut_ptr(&mut self) -> *mut T {
        self.0.as_mut_ptr()
    }

    pub const unsafe fn assume_init(self) -> T {
        self.0.assume_init()
    }
}

impl<T: Clone> Clone for DefaultUninit<T> {
    fn clone(&self) -> Self {
        unsafe { Self((&self.0 as *const MaybeUninit<T>).read()) }
    }
}

impl<T> Default for DefaultUninit<T> {
    fn default() -> Self {
        Self(MaybeUninit::uninit())
    }
}
