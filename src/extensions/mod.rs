/*
    Whilst writing panopticon-m365 I noticed several crucial bits of functionality missing from the core library.

    'Extensions' refers to a map of type-indexed values that can be used to extend the functionality of commands
    without requiring global statics or other awkward patterns. For example, panopticon-m365 uses this to store
    authentication tokens and HTTP clients that can be accessed by commands needing to make API calls.

    Additionally I'm going to introduce some traits that allow for extension categories starting with "UserInteraction", with a notify/prompt pattern.

    Super subject to change cause some of the functionality here requires I accept some runtime errors. I'm sure Descriptor couldddd expose a 'required-extensions' method.
    However I've been pretty good so far at avoiding runtime errors wherever possible.

    Doing this as a folder module for now, good chance it'll grow.
*/

use crate::imports::*;
use std::any::{Any, TypeId};

#[derive(Clone)]
pub struct Extensions {
    map: HashMap<TypeId, Arc<Box<dyn Any + Send + Sync>>>,
}

impl std::fmt::Debug for Extensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extensions")
            .field("keys", &self.map.keys().collect::<Vec<&TypeId>>())
            .finish()
    }
}

impl Default for Extensions {
    fn default() -> Self {
        let mut ext = Extensions {
            map: HashMap::new(),
        };
        ext.insert::<tokio_util::sync::CancellationToken>(
            tokio_util::sync::CancellationToken::new(),
        );
        ext
    }
}

impl Extensions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: Send + Sync + 'static>(&mut self, val: T) {
        self.map.insert(TypeId::of::<T>(), Arc::new(Box::new(val)));
    }

    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>())?.downcast_ref()
    }

    // Helper methods for checking CancellationToken
    pub fn cancel_token(&self) -> Option<&tokio_util::sync::CancellationToken> {
        self.get::<tokio_util::sync::CancellationToken>()
    }

    pub fn is_canceled(&self) -> bool {
        if let Some(token) = self.cancel_token() {
            token.is_cancelled()
        } else {
            false
        }
    }

    pub fn cancel(&self) {
        if let Some(token) = self.cancel_token() {
            token.cancel();
        }
    }
}
