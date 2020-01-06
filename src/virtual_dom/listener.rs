use super::Ev;
use crate::app::MessageMapper;
use crate::browser::util::ClosureNew;
use std::{fmt, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast};

// ------ EventHandler ------

pub struct EventHandler<Ms>(Rc<dyn Fn(web_sys::Event) -> Ms>);

impl<Ms, F: Fn(web_sys::Event) -> Ms + 'static> From<F> for EventHandler<Ms> {
    fn from(func: F) -> Self {
        EventHandler(Rc::new(func))
    }
}

impl<Ms> Clone for EventHandler<Ms> {
    fn clone(&self) -> Self {
        EventHandler(self.0.clone())
    }
}

impl<Ms> PartialEq for EventHandler<Ms> {
    fn eq(&self, other: &EventHandler<Ms>) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<Ms> EventHandler<Ms> {
    pub fn call(&self, event: web_sys::Event) -> Ms {
        (self.0)(event)
    }
}

impl<Ms: 'static, OtherMs: 'static> MessageMapper<Ms, OtherMs> for EventHandler<Ms> {
    type SelfWithOtherMs = EventHandler<OtherMs>;
    fn map_msg(
        self,
        msg_mapper: impl FnOnce(Ms) -> OtherMs + 'static + Clone,
    ) -> EventHandler<OtherMs> {
        let orignal_handler = self;
        let new_handler = move |event| {
            let msg = orignal_handler.call(event);
            (msg_mapper.clone())(msg)
        };
        EventHandler(Rc::new(new_handler))
    }
}

impl<Ms> fmt::Debug for EventHandler<Ms> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "EventHandler")
    }
}

// ------ Listener ------

/// Ev-handling for Elements
#[derive(Debug)]
pub struct Listener<Ms> {
    pub trigger: Ev,
    // Handler describes how to handle the event, and is used to generate the closure.
    pub handler: Option<EventHandler<Ms>>,
    // We store closure here so we can detach it later.
    pub closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
    // Control listeners prevent input on controlled input elements, and
    // are not assoicated with a message.
    pub control_val: Option<String>,
    pub control_checked: Option<bool>,
}

impl<Ms> Clone for Listener<Ms> {
    fn clone(&self) -> Self {
        Self {
            trigger: self.trigger,
            handler: self.handler.clone(),
            // closure shouldn't be cloned since the new Listener isn't related to this Listener
            closure: None,
            control_val: self.control_val.clone(),
            control_checked: self.control_checked,
        }
    }
}

impl<Ms> Listener<Ms> {
    pub fn new(trigger: &str, handler: Option<impl Into<EventHandler<Ms>>>) -> Self {
        Self {
            // We use &str instead of Event here to allow flexibility in helper funcs,
            // without macros by using ToString.
            trigger: trigger.into(),
            handler: handler.map(Into::into),
            closure: None,
            control_val: None,
            control_checked: None,
        }
    }

    /// Set up a listener that keeps the field's value in sync with the specific value,
    /// from the model
    pub fn new_control(val: String) -> Self {
        Self {
            trigger: Ev::Input,
            handler: None,
            closure: None,
            control_val: Some(val),
            control_checked: None,
        }
    }

    /// Similar to `new_control`, but for checkboxes
    pub fn new_control_check(checked: bool) -> Self {
        Self {
            trigger: Ev::Click,
            handler: None,
            closure: None,
            control_val: None,
            control_checked: Some(checked),
        }
    }

    /// This method is where the processing logic for events happens.
    pub fn attach<T>(&mut self, el_ws: &T, mailbox: crate::virtual_dom::mailbox::Mailbox<Ms>)
    where
        T: AsRef<web_sys::EventTarget>,
    {
        let handler = self.handler.clone().expect("Can't find old handler");
        // This is the closure ran when a DOM element has an user defined callback
        let closure = Closure::new(move |event: web_sys::Event| {
            let msg = handler.call(event);
            mailbox.send(msg);
        });

        (el_ws.as_ref() as &web_sys::EventTarget)
            .add_event_listener_with_callback(
                self.trigger.as_str(),
                closure.as_ref().unchecked_ref(),
            )
            .expect("Problem adding listener to element");

        // Store the closure so we can detach it later. Not detaching it when an element
        // is removed will trigger a panic.
        if self.closure.replace(closure).is_some() {
            panic!("self.closure already set in attach");
        }
    }

    pub fn detach<T>(&mut self, el_ws: &T)
    where
        T: AsRef<web_sys::EventTarget>,
    {
        let closure = self.closure.take().expect("Can't find closure to detach");

        (el_ws.as_ref() as &web_sys::EventTarget)
            .remove_event_listener_with_callback(
                self.trigger.as_str(),
                closure.as_ref().unchecked_ref(),
            )
            .expect("Problem removing listener from element");
    }
}

impl<Ms> PartialEq for Listener<Ms> {
    fn eq(&self, other: &Self) -> bool {
        self.trigger == other.trigger
    }
}

impl<Ms: 'static, OtherMs: 'static> MessageMapper<Ms, OtherMs> for Listener<Ms> {
    type SelfWithOtherMs = Listener<OtherMs>;
    fn map_msg(self, f: impl FnOnce(Ms) -> OtherMs + 'static + Clone) -> Listener<OtherMs> {
        Listener {
            trigger: self.trigger,
            handler: self.handler.map(|event| event.map_msg(f.clone())),
            closure: self.closure,
            control_val: self.control_val,
            control_checked: self.control_checked,
        }
    }
}
