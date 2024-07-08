use fmc::items::{Item, ItemId, ItemStack};

#[derive(Hash, PartialEq, Eq)]
pub struct Pattern {
    pub(super) inner: Vec<Vec<Option<ItemId>>>,
}

/// Convert a list of items into the smallest list that can represent the grid layout of the items.
/// i.e a list like:
///     [0,0,1
///      0,1,0
///      0,0,0]
/// can be compressed to the Pattern:
///     [
///         [0,1]
///         [1,0]
///     ]
/// which is the 2x2 square in the top right corner.
/// Grid inputs that are not a square will panic.
impl From<&[ItemStack]> for Pattern {
    fn from(grid: &[ItemStack]) -> Self {
        let grid_size = (grid.len() as f32).sqrt() as usize;

        // TODO: I don't know if there can be errors in the above calculation.
        assert!(grid_size.pow(2) == grid.len());

        let mut max_row = 0;
        let mut max_col = 0;
        let mut min_row = grid_size;
        let mut min_col = grid_size;

        for (i, row) in grid.chunks(grid_size).enumerate() {
            for (j, item_stack) in row.iter().enumerate() {
                if !item_stack.is_empty() {
                    if i > max_row {
                        max_row = i;
                    }
                    if i < min_row {
                        min_row = i;
                    }
                    if j > max_col {
                        max_col = j;
                    }
                    if j < min_col {
                        min_col = j;
                    }
                }
            }
        }

        let mut inner = Vec::new();

        for row in grid.chunks(grid_size).take(max_row + 1).skip(min_row) {
            let mut pattern_row = Vec::new();

            for item_stack in row.iter().take(max_col + 1).skip(min_col) {
                if let Some(item) = item_stack.item() {
                    pattern_row.push(Some(item.id));
                } else {
                    pattern_row.push(None);
                }
            }

            inner.push(pattern_row);
        }

        return Pattern { inner };
    }
}

/// A recipe is counterpart to a `Pattern` and holds how many of each item in the pattern is needed
/// to create the `Item`.
pub struct Recipe {
    pub(super) required_amount: Vec<Vec<u32>>,
    pub(super) output_item: Item,
    pub(super) output_amount: u32,
    pub(super) data: serde_json::Value,
}

// XXX: The functions that are pub(super) require that the 'input' parameter matches the recipe
// pattern. They can therefore not be used independently and are always used through the
// super::RecipeCollection struct which checks beforehand.
impl Recipe {
    pub(super) fn craft(&self, input: &mut [ItemStack], mut amount: u32) -> Option<(Item, u32)> {
        // The given amount is the amount of items requested j
        amount = std::cmp::min(
            amount / self.output_amount,
            self.get_craftable_amount(input) / self.output_amount,
        );

        input
            .iter_mut()
            .filter(|x| !x.is_empty())
            .zip(self.required_amount.iter().flatten().filter(|&x| *x > 0))
            .for_each(|(item_stack, required)| {
                item_stack.subtract(required * amount);
            });

        return Some((self.output_item.clone(), amount * self.output_amount));
    }

    /// Get how many of the crafting output it is possible to make.
    pub(super) fn get_craftable_amount(&self, input: &[ItemStack]) -> u32 {
        let mut amount_can_craft = u32::MAX;

        // The input slice is sometimes longer than the required input vector. This isn't a problem
        // as the amount of positive numbers in the required input vector will always match the
        // amount of non empty itemstacks in the input, and they will always occur in the same
        // order.
        input
            .iter()
            .filter(|&x| !x.is_empty())
            .zip(self.required_amount.iter().flatten().filter(|&x| *x > 0))
            .for_each(|(item, required)| {
                let can_craft = item.size() / required;
                if can_craft < amount_can_craft {
                    amount_can_craft = can_craft;
                }
            });

        if amount_can_craft < u32::MAX {
            return amount_can_craft * self.output_amount;
        } else {
            return 0;
        }
    }

    pub fn output_item(&self) -> &Item {
        return &self.output_item;
    }

    pub fn data(&self) -> &serde_json::Value {
        return &self.data;
    }
}
