//! Backend implementation for a glutin headless renderer.

use crate::{Frame, IncompatibleOpenGl};
use crate::debug;
use crate::context;
use crate::backend::{self, Backend};
use std::ffi::CString;
use std::rc::Rc;
use std::cell::{Ref, RefCell, Cell};
use std::ops::Deref;
use std::os::raw::c_void;
use super::glutin::display::GetGlDisplay;
use super::glutin::prelude::*;
use super::glutin::context::PossiblyCurrentContext;
use takeable_option::Takeable;

/// A headless glutin context.
pub struct Headless {
    context: Rc<context::Context>,
    glutin: Rc<RefCell<Takeable<PossiblyCurrentContext>>>,
    framebuffer_dimensions: Cell<(u32, u32)>,
}

/// An implementation of the `Backend` trait for a glutin headless context.
pub struct GlutinBackend(Rc<RefCell<Takeable<PossiblyCurrentContext>>>);

impl Deref for Headless {
    type Target = context::Context;
    fn deref(&self) -> &context::Context {
        &self.context
    }
}

impl Deref for GlutinBackend {
    type Target = Rc<RefCell<Takeable<PossiblyCurrentContext>>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

unsafe impl Backend for GlutinBackend {
    #[inline]
    unsafe fn get_proc_address(&self, symbol: &str) -> *const c_void {
        let symbol = CString::new(symbol).unwrap();
        let ret = self.0.borrow().display().get_proc_address(&symbol) as *const _;
        println!("{:?}", ret);
        ret
    }

    #[inline]
    fn get_framebuffer_dimensions(&self) -> (u32, u32) {
        todo!();
    }

    #[inline]
    fn set_framebuffer_dimensions(&self, new_dimensions: (u32, u32)) {
        todo!();
    }

    #[inline]
    fn is_current(&self) -> bool {
        self.0.borrow().is_current()
    }
}

impl backend::Facade for Headless {
    #[inline]
    fn get_context(&self) -> &Rc<context::Context> {
        &self.context
    }
}

impl Headless {
    /// Create a new glium `Headless` context.
    ///
    /// Performs a compatibility check to make sure that all core elements of glium are supported
    /// by the implementation.
    pub fn new(context: PossiblyCurrentContext) -> Result<Self, IncompatibleOpenGl> {
        Self::with_debug(context, Default::default())
    }

    /// Create a new glium `Headless` context.
    ///
    /// This function does the same as `build_glium`, except that the resulting context
    /// will assume that the current OpenGL context will never change.
    pub unsafe fn unchecked(context: PossiblyCurrentContext) -> Result<Self, IncompatibleOpenGl> {
        Self::unchecked_with_debug(context, Default::default())
    }

    /// The same as the `new` constructor, but allows for specifying debug callback behaviour.
    pub fn with_debug(context: PossiblyCurrentContext, debug: debug::DebugCallbackBehavior)
        -> Result<Self, IncompatibleOpenGl>
    {
        Self::new_inner(context, debug, true)
    }

    /// The same as the `unchecked` constructor, but allows for specifying debug callback behaviour.
    pub unsafe fn unchecked_with_debug(
        context: PossiblyCurrentContext,
        debug: debug::DebugCallbackBehavior,
    ) -> Result<Self, IncompatibleOpenGl>
    {
        Self::new_inner(context, debug, false)
    }

    fn new_inner(
        context: PossiblyCurrentContext,
        debug: debug::DebugCallbackBehavior,
        checked: bool,
    ) -> Result<Self, IncompatibleOpenGl>
    {
        let glutin_context = Rc::new(RefCell::new(Takeable::new(context)));
        let glutin_backend = GlutinBackend(glutin_context.clone());
        let context = unsafe { context::Context::new(glutin_backend, checked, debug) }?;
        let framebuffer_dimensions = Cell::new((800, 600));
        Ok(Headless { context, glutin: glutin_context, framebuffer_dimensions })
    }

    /// Borrow the inner glutin context
    pub fn gl_context(&self) -> Ref<'_, impl Deref<Target = PossiblyCurrentContext>> {
        self.glutin.borrow()
    }

    /// Start drawing on the backbuffer.
    ///
    /// This function returns a `Frame`, which can be used to draw on it. When the `Frame` is
    /// destroyed, the buffers are swapped.
    ///
    /// Note that destroying a `Frame` is immediate, even if vsync is enabled.
    ///
    /// If the framebuffer dimensions have changed since the last call to `draw`, the inner glutin
    /// context will be resized accordingly before returning the `Frame`.
    #[inline]
    pub fn draw(&self) -> Frame {
        Frame::new(self.context.clone(), self.get_framebuffer_dimensions())
    }
}
