use super::{
    gamelog::Gamelog, AreaOfEffect, CombatStats, Confusion, Consumable, Equippable, Equipped,
    InBackpack, InflictsDamage, Map, Name, Position, ProvidesHealing, SufferDamage,
    WantsToDropItem, WantsToPickupItem, WantsToUseItem,
};
use specs::prelude::*;

pub struct ItemCollectionSystem;

impl<'a> System<'a> for ItemCollectionSystem {
    type SystemData = (
        ReadExpect<'a, Entity>,
        WriteExpect<'a, Gamelog>,
        WriteStorage<'a, WantsToPickupItem>,
        WriteStorage<'a, Position>,
        ReadStorage<'a, Name>,
        WriteStorage<'a, InBackpack>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (player_entity, mut gamelog, mut wants_pickup, mut positions, names, mut backpack) =
            data;

        for pickup in wants_pickup.join() {
            positions.remove(pickup.item);
            backpack
                .insert(
                    pickup.item,
                    InBackpack {
                        owner: pickup.collected_by,
                    },
                )
                .expect("Unable to insert backpack entry");

            if pickup.collected_by == *player_entity {
                gamelog.entries.push(format!(
                    "You pick up the {}.",
                    names
                        .get(pickup.item)
                        .expect("Failed to get item name")
                        .name
                ));
            }
        }

        wants_pickup.clear();
    }
}

pub struct ItemUseSystem;
impl<'a> System<'a> for ItemUseSystem {
    type SystemData = (
        ReadExpect<'a, Entity>,
        WriteExpect<'a, Gamelog>,
        Entities<'a>,
        WriteStorage<'a, Consumable>,
        WriteStorage<'a, WantsToUseItem>,
        ReadStorage<'a, Name>,
        ReadStorage<'a, ProvidesHealing>,
        ReadStorage<'a, InflictsDamage>,
        WriteStorage<'a, CombatStats>,
        ReadExpect<'a, Map>,
        WriteStorage<'a, SufferDamage>,
        ReadStorage<'a, AreaOfEffect>,
        WriteStorage<'a, Confusion>,
        WriteStorage<'a, Equipped>,
        ReadStorage<'a, Equippable>,
        WriteStorage<'a, InBackpack>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (
            player_entity,
            mut gamelog,
            entities,
            consumables,
            mut wants_use,
            names,
            healers,
            damagers,
            mut combat_stats,
            map,
            mut suffer_damage,
            aoe,
            mut confusers,
            mut equipped,
            equippable,
            mut backpack,
        ) = data;

        for (entity, wants_use) in (&entities, &wants_use).join() {
            // Targeting
            let mut targets: Vec<Entity> = Vec::new();
            match wants_use.target {
                None => {
                    targets.push(*player_entity);
                }
                Some(target) => {
                    match aoe.get(wants_use.item) {
                        None => {
                            // Single target in tile
                            let idx = map.xy_idx(target.x, target.y);
                            for mob in map.tile_content[idx].iter() {
                                targets.push(*mob);
                            }
                        }
                        Some(aoe) => {
                            // AoE
                            let mut blast_tiles = rltk::field_of_view(target, aoe.radius, &*map);
                            blast_tiles.retain(|p| {
                                p.x > 0 && p.x < map.width - 1 && p.y > 0 && p.y < map.height - 1
                            });
                            for tile_idx in blast_tiles.iter() {
                                let idx = map.xy_idx(tile_idx.x, tile_idx.y);
                                for mob in map.tile_content[idx].iter() {
                                    targets.push(*mob);
                                }
                            }
                        }
                    }
                }
            }

            // If it is equippable, then we want to equip it - and unequip whatever else was in that slot
            let item_equippable = equippable.get(wants_use.item);
            if let Some(can_equip) = item_equippable {
                let target_slot = can_equip.slot;
                let target = targets[0];

                // Remove any items the target has in the item's slot
                let mut to_unequip: Vec<Entity> = Vec::new();
                for (item_entity, already_equipped, name) in (&entities, &equipped, &names).join() {
                    if already_equipped.owner == target && already_equipped.slot == target_slot {
                        to_unequip.push(item_entity);
                        if target == *player_entity {
                            gamelog
                                .entries
                                .push(format!("You unequip {item_name}.", item_name = name.name));
                        }
                    }
                }
                for item in to_unequip.iter() {
                    equipped.remove(*item);
                    backpack
                        .insert(*item, InBackpack { owner: target })
                        .expect("Unable to insert backpack entry");
                }

                // Wield the item
                equipped
                    .insert(
                        wants_use.item,
                        Equipped {
                            owner: target,
                            slot: target_slot,
                        },
                    )
                    .expect("Unable to insert equipped component");
                backpack.remove(wants_use.item);
                if target == *player_entity {
                    gamelog.entries.push(format!(
                        "You equip {item_name}.",
                        item_name = names.get(wants_use.item).unwrap().name
                    ));
                }
            }

            // Damaging Item
            if let Some(damager) = damagers.get(wants_use.item) {
                for mob in targets.iter() {
                    SufferDamage::new_damage(&mut suffer_damage, *mob, damager.damage);
                    if entity == *player_entity {
                        gamelog.entries.push(format!(
                            "You use {item_name} on {mob_name}, inflicting {amount} hp.",
                            amount = damager.damage,
                            mob_name = names.get(*mob).unwrap().name,
                            item_name = names.get(wants_use.item).unwrap().name,
                        ));
                    }
                    // used_item = true;
                }
            }

            // Healing Item
            if let Some(healer) = healers.get(wants_use.item) {
                for target in targets.iter() {
                    if let Some(stats) = combat_stats.get_mut(*target) {
                        let amount = if stats.hp + healer.heal_amount > stats.max_hp {
                            stats.max_hp - stats.hp
                        } else {
                            healer.heal_amount
                        };
                        stats.hp = i32::min(stats.max_hp, stats.hp + healer.heal_amount);
                        if entity == *player_entity {
                            gamelog.entries.push(format!(
                                "You drink the {potion_name}, healing {amount} hp.",
                                potion_name = names.get(wants_use.item).unwrap().name,
                            ));
                        }
                    }
                }
            }

            // Confusion Item
            // map To avoid double borrow
            if let Some(turns) = confusers.get(wants_use.item).map(|confuser| confuser.turns) {
                for mob in targets.iter() {
                    confusers
                        .insert(*mob, Confusion { turns })
                        .expect("Unable to insert status");
                    if entity == *player_entity {
                        gamelog.entries.push(format!(
                            "You use {item_name} on {mob_name}, confusing them.",
                            mob_name = names.get(*mob).unwrap().name,
                            item_name = names.get(wants_use.item).unwrap().name,
                        ));
                    }
                }
            }

            if consumables.get(wants_use.item).is_some() {
                entities.delete(wants_use.item).expect("Delete failed");
            }
        }

        wants_use.clear();
    }
}

pub struct ItemDropSystem;

impl<'a> System<'a> for ItemDropSystem {
    type SystemData = (
        ReadExpect<'a, Entity>,
        WriteExpect<'a, Gamelog>,
        Entities<'a>,
        WriteStorage<'a, WantsToDropItem>,
        ReadStorage<'a, Name>,
        WriteStorage<'a, Position>,
        WriteStorage<'a, InBackpack>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (
            player_entity,
            mut gamelog,
            entities,
            mut wants_drop,
            names,
            mut positions,
            mut backpack,
        ) = data;

        for (entity, to_drop) in (&entities, &wants_drop).join() {
            let dropper_pos = positions.get(entity).unwrap().clone();

            positions
                .insert(to_drop.item, dropper_pos)
                .expect("Unable to insert position");
            backpack.remove(to_drop.item);

            if entity == *player_entity {
                gamelog.entries.push(format!(
                    "You drop the {item_name}.",
                    item_name = names.get(to_drop.item).unwrap().name
                ));
            }
        }
        wants_drop.clear();
    }
}
