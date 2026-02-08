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
use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};

/// Read-only guard for accessing extensions
pub struct ExtensionsReadGuard<'a> {
    guard: RwLockReadGuard<'a, HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl<'a> ExtensionsReadGuard<'a> {
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.guard.get(&TypeId::of::<T>())?.downcast_ref()
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.guard.contains_key(&TypeId::of::<T>())
    }
}

/// Read-write guard for modifying extensions
pub struct ExtensionsWriteGuard<'a> {
    guard: RwLockWriteGuard<'a, HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl<'a> ExtensionsWriteGuard<'a> {
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.guard.get(&TypeId::of::<T>())?.downcast_ref()
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.guard.get_mut(&TypeId::of::<T>())?.downcast_mut()
    }

    pub fn insert<T: Send + Sync + 'static>(&mut self, val: T) {
        self.guard.insert(TypeId::of::<T>(), Box::new(val));
    }

    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.guard
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast().ok())
            .map(|b| *b)
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.guard.contains_key(&TypeId::of::<T>())
    }
}

#[derive(Clone)]
pub struct Extensions {
    map: Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    keys: Vec<TypeId>, // For Debug purposes, since we can't use the async method to read it during Debug impl
}

impl std::fmt::Debug for Extensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extensions")
            .field("keys", &self.keys)
            .finish()
    }
}

impl Default for Extensions {
    fn default() -> Self {
        let mut map = HashMap::new();
        map.insert(
            TypeId::of::<tokio_util::sync::CancellationToken>(),
            Box::new(tokio_util::sync::CancellationToken::new()) as Box<dyn Any + Send + Sync>,
        );
        Extensions {
            map: Arc::new(RwLock::new(map)),
            keys: vec![TypeId::of::<tokio_util::sync::CancellationToken>()],
        }
    }
}

impl Extensions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Acquire a read lock on the extensions map
    pub async fn read(&self) -> ExtensionsReadGuard<'_> {
        ExtensionsReadGuard {
            guard: self.map.read().await,
        }
    }

    /// Acquire a write lock on the extensions map
    pub async fn write(&self) -> ExtensionsWriteGuard<'_> {
        ExtensionsWriteGuard {
            guard: self.map.write().await,
        }
    }

    // Convenience methods for CancellationToken

    pub async fn is_canceled(&self) -> bool {
        self.read()
            .await
            .get::<tokio_util::sync::CancellationToken>()
            .map(|t| t.is_cancelled())
            .unwrap_or(false)
    }

    pub async fn cancel(&self) {
        if let Some(token) = self
            .read()
            .await
            .get::<tokio_util::sync::CancellationToken>()
        {
            token.cancel();
        }
    }
}
