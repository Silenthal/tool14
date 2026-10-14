#[derive(Default, Debug, Clone, Copy)]
pub struct ItemDetail {
    pub item_id: i32,
    pub count: i32,
    pub is_hq: bool,
}

impl PartialEq for ItemDetail {
    fn eq(&self, other: &Self) -> bool {
        self.item_id == other.item_id && self.is_hq == other.is_hq
    }
}

impl ItemDetail {
    pub fn new(item_id: i32, count: i32, is_hq: bool) -> Self {
        Self {
            item_id,
            count,
            is_hq,
        }
    }
}
