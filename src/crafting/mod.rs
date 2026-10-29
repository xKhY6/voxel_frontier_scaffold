use serde::{Deserialize, Deserializer, Serialize};

use crate::voxel::block::BlockType;

const STACK_LIMIT: u16 = 999;
const RECIPE_DATA: &str = include_str!("../../assets/recipes.json");

/// Ressources transportées par le joueur, limitées à 999 unités par matériau.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    #[serde(default = "empty_counts", deserialize_with = "deserialize_counts")]
    counts: [u16; 9],
}

fn empty_counts() -> [u16; 9] {
    [0; 9]
}

fn deserialize_counts<'de, D>(deserializer: D) -> Result<[u16; 9], D::Error>
where
    D: Deserializer<'de>,
{
    let counts = Vec::<u16>::deserialize(deserializer)?;
    match counts.len() {
        4 => Ok([counts[0], counts[1], counts[2], counts[3], 0, 0, 0, 0, 0]),
        6 => Ok([
            counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], 0, 0, 0,
        ]),
        8 => Ok([
            counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6], counts[7],
            0,
        ]),
        9 => Ok([
            counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6], counts[7],
            counts[8],
        ]),
        _ => Err(serde::de::Error::custom(
            "l'inventaire doit contenir 4, 6, 8 ou 9 piles",
        )),
    }
}

impl Inventory {
    pub(crate) fn is_valid(&self) -> bool {
        self.counts.iter().all(|count| *count <= STACK_LIMIT)
    }

    pub fn count(&self, block: BlockType) -> u16 {
        item_index(block).map_or(0, |index| self.counts[index])
    }

    pub fn can_add(&self, block: BlockType, amount: u16) -> bool {
        item_index(block).is_some_and(|index| {
            self.counts[index]
                .checked_add(amount)
                .is_some_and(|sum| sum <= STACK_LIMIT)
        })
    }

    pub fn add(&mut self, block: BlockType, amount: u16) -> bool {
        let Some(index) = item_index(block) else {
            return false;
        };
        let Some(sum) = self.counts[index].checked_add(amount) else {
            return false;
        };
        if sum > STACK_LIMIT {
            return false;
        }
        self.counts[index] = sum;
        true
    }

    pub fn remove(&mut self, block: BlockType, amount: u16) -> bool {
        let Some(index) = item_index(block) else {
            return false;
        };
        let Some(remaining) = self.counts[index].checked_sub(amount) else {
            return false;
        };
        self.counts[index] = remaining;
        true
    }

    /// Fabrique la recette identifiée dans `assets/recipes.json`.
    pub fn craft(&mut self, recipe_id: &str) -> Result<(), CraftError> {
        let recipes: Vec<Recipe> = serde_json::from_str(RECIPE_DATA)
            .map_err(|error| CraftError::InvalidRecipeData(error.to_string()))?;
        let recipe = recipes
            .iter()
            .find(|recipe| recipe.id == recipe_id)
            .ok_or(CraftError::UnknownRecipe)?;
        self.craft_recipe(recipe)
    }

    fn craft_recipe(&mut self, recipe: &Recipe) -> Result<(), CraftError> {
        if recipe.ingredients.is_empty()
            || recipe.output.amount == 0
            || !recipe.output.block.is_solid()
            || recipe
                .ingredients
                .iter()
                .any(|ingredient| ingredient.amount == 0 || !ingredient.block.is_solid())
        {
            return Err(CraftError::InvalidRecipeData(
                "ingrédients et sortie doivent être des blocs solides en quantité positive"
                    .to_owned(),
            ));
        }

        for (index, ingredient) in recipe.ingredients.iter().enumerate() {
            if recipe.ingredients[..index]
                .iter()
                .any(|previous| previous.block == ingredient.block)
            {
                continue;
            }
            let required = recipe.ingredients[index..]
                .iter()
                .filter(|entry| entry.block == ingredient.block)
                .try_fold(0_u16, |total, entry| total.checked_add(entry.amount))
                .ok_or_else(|| {
                    CraftError::InvalidRecipeData("quantité d'ingrédient trop grande".to_owned())
                })?;
            if self.count(ingredient.block) < required {
                return Err(CraftError::MissingIngredients);
            }
        }

        let consumed_output = recipe
            .ingredients
            .iter()
            .filter(|ingredient| ingredient.block == recipe.output.block)
            .try_fold(0_u16, |total, ingredient| {
                total.checked_add(ingredient.amount)
            })
            .ok_or_else(|| {
                CraftError::InvalidRecipeData("quantité d'ingrédient trop grande".to_owned())
            })?;
        let resulting_output = self
            .count(recipe.output.block)
            .checked_sub(consumed_output)
            .and_then(|remaining| remaining.checked_add(recipe.output.amount));
        if !resulting_output.is_some_and(|amount| amount <= STACK_LIMIT) {
            return Err(CraftError::InventoryFull);
        }

        for ingredient in &recipe.ingredients {
            if !self.remove(ingredient.block, ingredient.amount) {
                return Err(CraftError::MissingIngredients);
            }
        }
        if !self.add(recipe.output.block, recipe.output.amount) {
            return Err(CraftError::InventoryFull);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CraftError {
    MissingIngredients,
    InventoryFull,
    UnknownRecipe,
    InvalidRecipeData(String),
}

#[derive(Debug, Deserialize)]
struct Recipe {
    id: String,
    ingredients: Vec<Ingredient>,
    output: Ingredient,
}

#[derive(Debug, Deserialize)]
struct Ingredient {
    block: BlockType,
    amount: u16,
}

fn item_index(block: BlockType) -> Option<usize> {
    match block {
        BlockType::Grass => Some(0),
        BlockType::Dirt => Some(1),
        BlockType::Stone => Some(2),
        BlockType::Brick => Some(3),
        BlockType::Sand => Some(4),
        BlockType::Snow => Some(5),
        BlockType::Wood => Some(6),
        BlockType::Leaves => Some(7),
        BlockType::Planks => Some(8),
        BlockType::Air => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_blocs_mines_peuvent_etre_ajoutes_et_depenses() {
        let mut inventory = Inventory::default();

        assert!(inventory.add(BlockType::Stone, 3));
        assert_eq!(inventory.count(BlockType::Stone), 3);
        assert!(inventory.remove(BlockType::Stone, 2));
        assert_eq!(inventory.count(BlockType::Stone), 1);
        assert!(!inventory.remove(BlockType::Stone, 2));
        assert!(inventory.add(BlockType::Wood, 2));
        assert!(inventory.add(BlockType::Leaves, 3));
        assert_eq!(inventory.count(BlockType::Wood), 2);
        assert_eq!(inventory.count(BlockType::Leaves), 3);
        assert!(!inventory.add(BlockType::Air, 1));
    }

    #[test]
    fn les_piles_sont_bornees_sans_perdre_de_ressources() {
        let mut inventory = Inventory::default();

        assert!(inventory.add(BlockType::Dirt, STACK_LIMIT));
        assert!(!inventory.add(BlockType::Dirt, 1));
        assert_eq!(inventory.count(BlockType::Dirt), STACK_LIMIT);
    }

    #[test]
    fn la_recette_donnees_consomme_les_ingredients_et_produit_quatre_briques() {
        let mut inventory = Inventory::default();
        inventory.add(BlockType::Stone, 2);
        inventory.add(BlockType::Dirt, 3);

        inventory.craft("stone_bricks").unwrap();

        assert_eq!(inventory.count(BlockType::Stone), 0);
        assert_eq!(inventory.count(BlockType::Dirt), 1);
        assert_eq!(inventory.count(BlockType::Brick), 4);
    }

    #[test]
    fn la_recette_de_planches_transforme_une_buche_en_quatre_planches() {
        let mut inventory = Inventory::default();
        inventory.add(BlockType::Wood, 2);

        inventory.craft("wood_planks").unwrap();

        assert_eq!(inventory.count(BlockType::Wood), 1);
        assert_eq!(inventory.count(BlockType::Planks), 4);
    }

    #[test]
    fn une_recette_invalide_ne_modifie_pas_l_inventaire() {
        let mut inventory = Inventory::default();
        inventory.add(BlockType::Stone, 2);
        let before = inventory.clone();

        assert_eq!(
            inventory.craft("stone_bricks"),
            Err(CraftError::MissingIngredients)
        );
        assert_eq!(inventory, before);
    }

    #[test]
    fn une_sortie_pleine_ne_consomme_pas_les_ingredients() {
        let mut inventory = Inventory::default();
        inventory.add(BlockType::Stone, 2);
        inventory.add(BlockType::Dirt, 2);
        inventory.add(BlockType::Brick, STACK_LIMIT - 3);
        let before = inventory.clone();

        assert_eq!(
            inventory.craft("stone_bricks"),
            Err(CraftError::InventoryFull)
        );
        assert_eq!(inventory, before);
    }

    #[test]
    fn une_recette_inconnue_ne_modifie_pas_l_inventaire() {
        let mut inventory = Inventory::default();
        inventory.add(BlockType::Stone, 2);
        let before = inventory.clone();

        assert_eq!(inventory.craft("unknown"), Err(CraftError::UnknownRecipe));
        assert_eq!(inventory, before);
    }
}
