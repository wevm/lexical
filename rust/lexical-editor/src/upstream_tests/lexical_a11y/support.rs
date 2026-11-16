//! Helpers shared by the ports of `@lexical/a11y`'s announcement tests (and
//! `@lexical/rich-text`'s `HeadingAnnounceExtension` test): a document body
//! that hosts live regions, and a root element in it.

use crate::a11y::{LiveRegion, LiveRegionHost};
use crate::{Editor, RootElement, RootElementRef};
use std::cell::RefCell;
use std::rc::Rc;

/// A document's `body`: the live regions mounted in it, in document order,
/// so `body.querySelectorAll('[aria-live]')` is [`Body::regions`].
#[derive(Default)]
pub(crate) struct Body {
    regions: RefCell<Vec<Rc<LiveRegion>>>,
}

impl Body {
    pub(crate) fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// `body.querySelectorAll('[aria-live]')`.
    pub(crate) fn regions(&self) -> Vec<Rc<LiveRegion>> {
        self.regions.borrow().clone()
    }

    /// `body.querySelector('[aria-live]')`.
    pub(crate) fn region(&self) -> Option<Rc<LiveRegion>> {
        self.regions.borrow().first().cloned()
    }

    /// `readLiveRegion()`: `body.querySelector('[aria-live]')!.textContent`.
    pub(crate) fn read_live_region(&self) -> String {
        self.region().expect("a live region").text_content()
    }
}

impl LiveRegionHost for Body {
    fn mount_live_region(&self, region: &Rc<LiveRegion>) {
        self.regions.borrow_mut().push(region.clone());
    }
    fn unmount_live_region(&self, region: &Rc<LiveRegion>) {
        self.regions.borrow_mut().retain(|r| !Rc::ptr_eq(r, region));
    }
}

/// A `contenteditable` div appended to a document's body: its live region
/// goes in that body (`ownerDocument.body`).
pub(crate) struct Root {
    body: Rc<Body>,
}

impl RootElement for Root {
    fn live_region_host(self: Rc<Self>) -> Option<Rc<dyn LiveRegionHost>> {
        Some(self.body.clone())
    }
}

/// `editor.setRootElement(root)` with a new root in `body`.
pub(crate) fn mount_root_in(editor: &Editor, body: &Rc<Body>) {
    editor
        .set_root_element(Some(RootElementRef(Rc::new(Root { body: body.clone() }))))
        .unwrap();
}

/// `mountRoot(editor)`: a new root in a new `document.body`, which it
/// returns.
pub(crate) fn mount_root(editor: &Editor) -> Rc<Body> {
    let body = Body::new();
    mount_root_in(editor, &body);
    body
}
