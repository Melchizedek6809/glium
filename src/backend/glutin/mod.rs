#![cfg(feature = "glutin")]
/*!

Backend implementation for the glutin library

# Features

Only available if the 'glutin' feature is enabled.

*/
pub use glutin;
use takeable_option::Takeable;

pub mod headless;

use crate::backend;
use crate::backend::Backend;
use crate::backend::Context;
use crate::context;
use crate::debug;
use crate::glutin::prelude::*;
use crate::glutin::context::{PossiblyCurrentContext, NotCurrentContext};
use crate::glutin::display::GetGlDisplay;
use crate::glutin::surface::{Surface, WindowSurface };
use std::cell::{Cell, Ref, RefCell};
use std::error::Error;
use std::ffi::CString;
use std::fmt;
use std::ops::Deref;
use std::os::raw::c_void;
use std::rc::Rc;
use crate::{Frame, IncompatibleOpenGl};

/// Contains a GL contenxt, that may or may not be current
pub enum WindowedContext {
    /// Contains a GL contenxt, that may be current
    PossiblyCurrent {
        /// The context
        context: PossiblyCurrentContext,
        /// Needs to be updated by the client code that is managing the surface.
        framebuffer_dimensions: Cell<(u32, u32)>,
    },
    /// Contains a GL contenxt, that is not current
    NotCurrent {
        /// The context
        context: NotCurrentContext,
    }
}

impl From<NotCurrentContext> for WindowedContext {
    fn from(context: NotCurrentContext) -> Self {
        Self::NotCurrent { context }
    }
}

impl From<PossiblyCurrentContext> for WindowedContext {
    fn from(context: PossiblyCurrentContext) -> Self {
        let framebuffer_dimensions = Cell::new((800, 600));
        Self::PossiblyCurrent { context, framebuffer_dimensions }
    }
}

impl WindowedContext {

    /// Return the display associated with the context
    pub fn display(&self) -> glutin::display::Display {
        match self {
            WindowedContext::NotCurrent { context } => context.display(),
            WindowedContext::PossiblyCurrent { context, .. } => context.display(),
        }
    }

    /// Return whether the context is current or not
    pub fn is_current(&self) -> bool {
        match self {
            WindowedContext::NotCurrent { context } => false,
            WindowedContext::PossiblyCurrent { context, .. } => context.is_current(),
        }
    }

    /// Update the framebuffer dimensions
    pub fn set_framebuffer_dimensions(&self, new_dimensions:(u32, u32)) {
        match self {
            WindowedContext::PossiblyCurrent { framebuffer_dimensions, .. } => {
                framebuffer_dimensions.replace(new_dimensions);
            },
            _ => (),
        }
    }
}

/// A GL context combined with a facade for drawing upon.
///
/// The `Display` uses **glutin** for the **Window** and its associated GL **Context**.
///
/// These are stored alongside a glium-specific context.
#[derive(Clone)]
pub struct Display {
    // contains everything related to the current context and its state
    context: Rc<context::Context>,
    // The glutin Window alongside its associated GL Context.
    gl_window: Rc<RefCell<Takeable<WindowedContext>>>,
    // Used to check whether the framebuffer dimensions have changed between frames. If they have,
    // the glutin context must be resized accordingly.
    last_framebuffer_dimensions: Cell<(u32, u32)>,
}

/// An implementation of the `Backend` trait for glutin.
#[derive(Clone)]
pub struct GlutinBackend(Rc<RefCell<Takeable<WindowedContext>>>);

/// Error that can happen while creating a glium display.
#[derive(Debug)]
pub enum DisplayCreationError {
    /// An error has happened while creating the backend.
    GlutinError(glutin::error::Error),
    /// The OpenGL implementation is too old.
    IncompatibleOpenGl(IncompatibleOpenGl),
}

impl std::fmt::Debug for Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[glium::backend::glutin::Display]")
    }
}

impl Display {
    /// Create a new glium `Display` from the given context and window builders.
    ///
    /// Performs a compatibility check to make sure that all core elements of glium are supported
    /// by the implementation.
    pub fn new(
        context: PossiblyCurrentContext
    ) -> Result<Self, DisplayCreationError> {
        Self::from_current_context(context).map_err(From::from)
    }

    /// Create a new glium `Display`.
    ///
    /// Performs a compatibility check to make sure that all core elements of glium are supported
    /// by the implementation.
    pub fn from_current_context(
        context: PossiblyCurrentContext
    ) -> Result<Self, IncompatibleOpenGl> {
        Self::with_debug(context, Default::default())
    }

    /// Create a new glium `Display`.
    ///
    /// This function does the same as `build_glium`, except that the resulting context
    /// will assume that the current OpenGL context will never change.
    pub unsafe fn unchecked(
        context: PossiblyCurrentContext
    ) -> Result<Self, IncompatibleOpenGl> {
        Self::unchecked_with_debug(context, Default::default())
    }

    /// The same as the `new` constructor, but allows for specifying debug callback behaviour.
    pub fn with_debug(
        context: PossiblyCurrentContext,
        debug: debug::DebugCallbackBehavior,
    ) -> Result<Self, IncompatibleOpenGl> {
        Self::new_inner(context, debug, true)
    }

    /// The same as the `unchecked` constructor, but allows for specifying debug callback behaviour.
    pub unsafe fn unchecked_with_debug(
        context: PossiblyCurrentContext,
        debug: debug::DebugCallbackBehavior,
    ) -> Result<Self, IncompatibleOpenGl> {
        Self::new_inner(context, debug, false)
    }

    fn new_inner(
        context: PossiblyCurrentContext,
        debug: debug::DebugCallbackBehavior,
        checked: bool,
    ) -> Result<Self, IncompatibleOpenGl> {
        let gl_window = Rc::new(RefCell::new(Takeable::new(context.into())));
        let glutin_backend = GlutinBackend(gl_window.clone());
        let framebuffer_dimensions = (800, 600);
        let context = unsafe { context::Context::new(glutin_backend, checked, debug) }?;
        Ok(Display {
            gl_window,
            context,
            last_framebuffer_dimensions: Cell::new((0,0)),
        })
    }

    fn set_framebuffer_dimensions(
        &mut self,
        new_dimensions: (u32, u32)
    ) {
        if let WindowedContext::PossiblyCurrent { framebuffer_dimensions, .. } = self.gl_window.borrow().deref().deref() {
            framebuffer_dimensions.replace(new_dimensions);
        }
    }

    /// Borrow the inner glutin WindowedContext.
    #[inline]
    pub fn gl_window(&self) -> Ref<Takeable<WindowedContext>> {
        self.gl_window.borrow()
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
        let (w, h) = self.context.get_framebuffer_dimensions();

        // If the size of the framebuffer has changed, resize the context.
        if self.last_framebuffer_dimensions.get() != (w, h) {
            self.last_framebuffer_dimensions.set((w, h));
            //self.gl_window.borrow().resize(self.framebuffer_dimensions.get().into());
        }

        Frame::new(self.context.clone(), (w, h))
    }
}

impl fmt::Display for DisplayCreationError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            DisplayCreationError::GlutinError(err) => write!(fmt, "{}", err),
            DisplayCreationError::IncompatibleOpenGl(err) => write!(fmt, "{}", err),
        }
    }
}

impl Error for DisplayCreationError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            DisplayCreationError::GlutinError(ref err) => Some(err),
            DisplayCreationError::IncompatibleOpenGl(ref err) => Some(err),
        }
    }
}

impl From<glutin::error::Error> for DisplayCreationError {
    #[inline]
    fn from(err: glutin::error::Error) -> DisplayCreationError {
        DisplayCreationError::GlutinError(err)
    }
}

impl From<IncompatibleOpenGl> for DisplayCreationError {
    #[inline]
    fn from(err: IncompatibleOpenGl) -> DisplayCreationError {
        DisplayCreationError::IncompatibleOpenGl(err)
    }
}

impl Deref for Display {
    type Target = Context;
    #[inline]
    fn deref(&self) -> &Context {
        &self.context
    }
}

impl backend::Facade for Display {
    #[inline]
    fn get_context(&self) -> &Rc<Context> {
        &self.context
    }
}

impl Deref for GlutinBackend {
    type Target = Rc<RefCell<Takeable<WindowedContext>>>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

unsafe impl Backend for GlutinBackend {
    #[inline]
    fn prepare_to_swap_buffers(&self) -> Result<(), crate::SwapBuffersError> {
        Ok(())
    }

    #[inline]
    unsafe fn get_proc_address(&self, symbol: &str) -> *const c_void {
        let symbol = CString::new(symbol).unwrap();
        let ret = self.borrow().display().get_proc_address(&symbol) as *const _;
        ret
    }

    #[inline]
    fn get_framebuffer_dimensions(&self) -> (u32, u32) {
        if let WindowedContext::PossiblyCurrent { framebuffer_dimensions, .. } = self.0.borrow().deref().deref() {
            framebuffer_dimensions.get()
        } else {
            todo!();
        }
    }

    #[inline]
    fn set_framebuffer_dimensions(&self, new_dimensions: (u32, u32)) {
        if let WindowedContext::PossiblyCurrent { framebuffer_dimensions, .. } = self.0.borrow().deref().deref() {
            framebuffer_dimensions.replace(new_dimensions);
        }
    }

    #[inline]
    fn is_current(&self) -> bool {
        match self.borrow().deref().deref() {
            WindowedContext::NotCurrent { .. } => false,
            WindowedContext::PossiblyCurrent { context, ..} => {
                context.is_current()
            }
        }
    }

    #[inline]
    unsafe fn make_current(&self) {
        /*
        let mut gl_window_takeable = self.borrow_mut();
        let gl_window = Takeable::take(&mut gl_window_takeable);
        let new_gl_window = gl_window.make_current().unwrap();
        Takeable::insert(&mut gl_window_takeable, new_gl_window);
         */
    }
}

impl GlutinBackend {
    #[inline]
    unsafe fn make_current_with_surface(&self, surface: &Surface<WindowSurface>) {
        let mut ctx = self.borrow_mut();
        let context = Takeable::take(&mut ctx);
        match context {
            WindowedContext::NotCurrent { context } => {
                Takeable::insert(&mut ctx, context.make_current(surface).unwrap().into());
            },
            WindowedContext::PossiblyCurrent { context, .. } => {
                let context = context.make_not_current().unwrap();
                Takeable::insert(&mut ctx, context.make_current(surface).unwrap().into());
            }
        }
    }
}
