use std::collections::HashMap;

// Android reuses pointer IDs. GPUI requires a fresh ID for every new contact.
#[derive(Default)]
pub(crate) struct TouchIds {
    next: u64,
    active: HashMap<i32, u64>,
}
impl TouchIds {
    pub(crate) fn start(&mut self, pointer: i32) -> u64 {
        self.next = self.next.wrapping_add(1);
        self.active.insert(pointer, self.next);
        self.next
    }
    pub(crate) fn get(&self, pointer: i32) -> Option<u64> {
        self.active.get(&pointer).copied()
    }
    pub(crate) fn end(&mut self, pointer: i32) {
        self.active.remove(&pointer);
    }
    pub(crate) fn clear(&mut self) {
        self.active.clear();
    }
    pub(crate) fn drain(&mut self) -> Vec<u64> {
        self.active.drain().map(|(_, id)| id).collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_reuse_gets_a_new_contact_identity() {
        let mut ids = TouchIds::default();
        let first = ids.start(0);
        assert_eq!(ids.get(0), Some(first));
        ids.end(0);
        assert_eq!(ids.get(0), None);
        assert_ne!(ids.start(0), first);
    }
    #[test]
    fn cancellation_discards_all_active_contacts() {
        let mut ids = TouchIds::default();
        ids.start(0);
        ids.start(1);
        assert_eq!(ids.drain().len(), 2);
        ids.clear();
        assert!(ids.get(0).is_none() && ids.get(1).is_none());
    }
}
