use serde::Deserialize;

use crate::random::{Rng, UniformDistribution, WeightedIndex};

use super::{ItemId, ItemStack, Items};

#[derive(Debug, Deserialize)]
pub struct DropTableJson {
    probability: f32,
    weights: Vec<f32>,
    drops: Vec<DropJson>,
}

#[derive(Debug, Deserialize)]
struct DropJson {
    item_name: String,
    min: u32,
    max: u32,
}

#[derive(Clone, Debug)]
pub struct DropTable {
    probability: f32,
    weights: WeightedIndex<f32>,
    drops: Vec<(ItemId, UniformDistribution<u32>)>,
}

impl DropTable {
    pub fn empty() -> Self {
        Self {
            probability: 0.0,
            weights: WeightedIndex::new(Vec::new()).unwrap(),
            drops: Vec::new(),
        }
    }

    pub fn new(
        // probability to hit the drop table
        probability: f32,
        // Item, weight, min amount, max amount
        drops: &[(ItemId, f32, u32, u32)],
    ) -> Result<Self, String> {
        if drops.is_empty() {
            return Err("'drops' cannot be empty".to_owned());
        }

        let weights = match WeightedIndex::new(drops.iter().map(|d| &d.1)) {
            Ok(w) => w,
            Err(_) => return Err("Weights must be positive and above zero.".to_owned()),
        };

        let drops = drops
            .iter()
            .map(|(item_id, _weight, min, max)| (*item_id, UniformDistribution::new(*min, *max)))
            .collect();

        Ok(Self {
            probability: probability.max(0.0).min(1.0),
            weights,
            drops,
        })
    }

    pub fn from_json(json: &DropTableJson) -> Result<DropTable, String> {
        let items = Items::get();

        let mut drops = Vec::with_capacity(json.drops.len());
        for drop in json.drops.iter() {
            if let Some(item_id) = items.get_id(&drop.item_name) {
                drops.push((item_id, UniformDistribution::new(drop.min, drop.max)));
            } else {
                return Err(format!("No item by the name '{}'", drop.item_name));
            }
        }

        let weights = match WeightedIndex::new(&json.weights) {
            Ok(w) => w,
            Err(_) => return Err("Weights must be positive and above zero.".to_owned()),
        };

        Ok(Self {
            probability: json.probability.max(0.0).min(1.0),
            weights,
            drops,
        })
    }

    pub fn drop(&self, mut rng: &mut Rng) -> Option<ItemStack> {
        if rng.next_f32() > self.probability {
            return None;
        }

        let (item_id, distribution) = &self.drops[self.weights.sample(&mut rng)];
        let count = distribution.sample(&mut rng);
        if count == 0 {
            return None;
        }

        let mut item_stack = ItemStack::new(*item_id, count);
        if count > item_stack.capacity() {
            item_stack.set_capacity(count);
        }

        return Some(item_stack);
    }
}
