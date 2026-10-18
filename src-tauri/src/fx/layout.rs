//! How a chain's effects are arranged: in processing order, some of them in named groups.
//!
//! A [`Layout`] is a list of [`Item`]s, each an effect or a group of effects. Groups hold
//! effects only, so a group's effects are always neighbours and groups never nest. The
//! operations here are pure: each returns the next layout, or an error leaving it as it was.

use crate::fx::chain::SlotId;

/// Identifies an effect group, unique for the life of the app.
pub type GroupId = u32;

/// Longest group name, in characters.
pub const MAX_NAME: usize = 64;

/// One entry of a chain: an effect, or a group of effects processed in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Effect(SlotId),
    Group(Group),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    /// Never empty in a layout: an operation that empties a group removes it.
    pub slots: Vec<SlotId>,
}

pub type Layout = Vec<Item>;

/// Where [`move_effect`] puts an effect: position `index` of `group`'s effects, or of the
/// top-level items when `group` is `None`, counted after the effect is taken out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub group: Option<GroupId>,
    pub index: usize,
}

const NOT_IN_CHAIN: &str = "That effect isn't in the chain";
const NO_GROUP: &str = "That group isn't in the chain";

/// The effects of `layout` in processing order.
pub fn slot_order(layout: &[Item]) -> Vec<SlotId> {
    layout.iter().flat_map(item_slots).copied().collect()
}

fn item_slots(item: &Item) -> &[SlotId] {
    match item {
        Item::Effect(s) => std::slice::from_ref(s),
        Item::Group(g) => &g.slots,
    }
}

pub fn contains(layout: &[Item], slot: SlotId) -> bool {
    layout.iter().any(|i| item_slots(i).contains(&slot))
}

/// The groups of `layout`, in order.
pub fn groups(layout: &[Item]) -> impl Iterator<Item = &Group> {
    layout.iter().filter_map(|i| match i {
        Item::Group(g) => Some(g),
        Item::Effect(_) => None,
    })
}

/// The group holding `slot`, if any.
pub fn group_of(layout: &[Item], slot: SlotId) -> Option<&Group> {
    groups(layout).find(|g| g.slots.contains(&slot))
}

fn group_index(layout: &[Item], id: GroupId) -> Result<usize, String> {
    layout
        .iter()
        .position(|i| matches!(i, Item::Group(g) if g.id == id))
        .ok_or_else(|| NO_GROUP.to_string())
}

/// `name` trimmed, if it is a valid group name.
pub fn valid_name(name: &str) -> Option<String> {
    let name = name.trim();
    let len = name.chars().count();
    (1..=MAX_NAME).contains(&len).then(|| name.to_string())
}

/// "Group N" with the lowest N no group of `layout` is called.
pub fn next_name(layout: &[Item]) -> String {
    let used: Vec<&str> = groups(layout).map(|g| g.name.as_str()).collect();
    (1..)
        .map(|n| format!("Group {n}"))
        .find(|name| !used.contains(&name.as_str()))
        .expect("an unused name")
}

/// Takes `slot` out of `layout`, wherever it is. Groups it empties stay until
/// [`drop_empty`]. False if it wasn't there.
fn take(layout: &mut Layout, slot: SlotId) -> bool {
    for (i, item) in layout.iter_mut().enumerate() {
        match item {
            Item::Effect(s) if *s == slot => {
                layout.remove(i);
                return true;
            }
            Item::Group(g) => {
                if let Some(j) = g.slots.iter().position(|&s| s == slot) {
                    g.slots.remove(j);
                    return true;
                }
            }
            Item::Effect(_) => {}
        }
    }
    false
}

fn drop_empty(layout: &mut Layout) {
    layout.retain(|i| !matches!(i, Item::Group(g) if g.slots.is_empty()));
}

/// `layout` without `slot`, which may leave its group, removing the group if it was the last.
pub fn remove_effect(layout: &[Item], slot: SlotId) -> Result<Layout, String> {
    let mut next = layout.to_vec();
    if !take(&mut next, slot) {
        return Err(NOT_IN_CHAIN.to_string());
    }
    drop_empty(&mut next);
    Ok(next)
}

/// `layout` with `slot` moved to `to`. Moving the last effect out of a group removes it.
pub fn move_effect(layout: &[Item], slot: SlotId, to: Target) -> Result<Layout, String> {
    let mut next = layout.to_vec();
    if !take(&mut next, slot) {
        return Err(NOT_IN_CHAIN.to_string());
    }
    match to.group {
        None => next.insert(to.index.min(next.len()), Item::Effect(slot)),
        Some(id) => {
            let i = group_index(&next, id)?;
            let Item::Group(g) = &mut next[i] else {
                unreachable!()
            };
            g.slots.insert(to.index.min(g.slots.len()), slot);
        }
    }
    drop_empty(&mut next);
    Ok(next)
}

/// `layout` with the selected effects (`selected`, any order) in a new group `id`. The group
/// takes the place of the first selected effect in processing order, and its effects keep
/// their order. Selected effects leave the groups they were in, and a group left empty goes.
/// If the first selected effect was in a group that keeps some effects, the new group goes
/// just before that group when none of them came before it, and just after otherwise. The
/// new group is named "Group N" ([`next_name`]) unless `name` is given.
pub fn group(
    layout: &[Item],
    selected: &[SlotId],
    id: GroupId,
    name: Option<String>,
) -> Result<Layout, String> {
    if selected.is_empty() {
        return Err("No effects are selected".to_string());
    }
    if selected.iter().any(|&s| !contains(layout, s)) {
        return Err(NOT_IN_CHAIN.to_string());
    }
    let members: Vec<SlotId> = slot_order(layout)
        .into_iter()
        .filter(|s| selected.contains(s))
        .collect();
    let first = members[0];
    let mut next = Layout::new();
    let mut placed = false;
    let mut place = |next: &mut Layout| {
        if !placed {
            placed = true;
            next.push(Item::Group(Group {
                id,
                name: String::new(),
                slots: members.clone(),
            }));
        }
    };
    for item in layout {
        match item {
            Item::Effect(s) if *s == first => place(&mut next),
            Item::Effect(s) if selected.contains(s) => {}
            Item::Effect(_) => next.push(item.clone()),
            Item::Group(g) => {
                let pos = g.slots.iter().position(|&s| s == first);
                let rest: Vec<SlotId> = g
                    .slots
                    .iter()
                    .copied()
                    .filter(|s| !selected.contains(s))
                    .collect();
                let before = pos.is_some_and(|p| g.slots[..p].iter().any(|s| rest.contains(s)));
                if pos.is_some() && !before {
                    place(&mut next);
                }
                if !rest.is_empty() {
                    next.push(Item::Group(Group {
                        slots: rest,
                        ..g.clone()
                    }));
                }
                if pos.is_some() {
                    place(&mut next);
                }
            }
        }
    }
    let name = name.unwrap_or_else(|| next_name(&next));
    for item in &mut next {
        if let Item::Group(g) = item
            && g.id == id
        {
            g.name = name.clone();
        }
    }
    Ok(next)
}

/// `layout` with group `id` replaced by its effects, in place.
pub fn ungroup(layout: &[Item], id: GroupId) -> Result<Layout, String> {
    let i = group_index(layout, id)?;
    let mut next = layout.to_vec();
    let Item::Group(g) = next.remove(i) else {
        unreachable!()
    };
    next.splice(i..i, g.slots.into_iter().map(Item::Effect));
    Ok(next)
}

/// `layout` with group `id` and its effects removed.
pub fn remove_group(layout: &[Item], id: GroupId) -> Result<Layout, String> {
    let i = group_index(layout, id)?;
    let mut next = layout.to_vec();
    next.remove(i);
    Ok(next)
}

/// `layout` with group `id` moved to top-level position `index` (clamped), counted after
/// the group is taken out.
pub fn move_group(layout: &[Item], id: GroupId, index: usize) -> Result<Layout, String> {
    let i = group_index(layout, id)?;
    let mut next = layout.to_vec();
    let item = next.remove(i);
    next.insert(index.min(next.len()), item);
    Ok(next)
}

/// `layout` with group `id` called `name`, trimmed. Fails for an empty or too long name.
pub fn rename_group(layout: &[Item], id: GroupId, name: &str) -> Result<Layout, String> {
    let name = valid_name(name)
        .ok_or_else(|| format!("A group name must be 1 to {MAX_NAME} characters"))?;
    let i = group_index(layout, id)?;
    let mut next = layout.to_vec();
    let Item::Group(g) = &mut next[i] else {
        unreachable!()
    };
    g.name = name;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(s: SlotId) -> Item {
        Item::Effect(s)
    }

    fn grp(id: GroupId, name: &str, slots: &[SlotId]) -> Item {
        Item::Group(Group {
            id,
            name: name.into(),
            slots: slots.to_vec(),
        })
    }

    #[test]
    fn groups_neighbours_in_place() {
        let l = vec![fx(1), fx(2), fx(3), fx(4)];
        let out = group(&l, &[3, 2], 10, None).unwrap();
        assert_eq!(out, [fx(1), grp(10, "Group 1", &[2, 3]), fx(4)]);
        assert_eq!(slot_order(&out), [1, 2, 3, 4]);
    }

    #[test]
    fn groups_effects_that_arent_neighbours_at_the_first() {
        let l = vec![fx(1), fx(2), fx(3)];
        let out = group(&l, &[1, 3], 10, None).unwrap();
        assert_eq!(out, [grp(10, "Group 1", &[1, 3]), fx(2)]);
    }

    #[test]
    fn grouping_across_groups_goes_after_what_stays() {
        let l = vec![grp(1, "Group 1", &[1, 2]), fx(3)];
        let out = group(&l, &[2, 3], 10, None).unwrap();
        assert_eq!(out, [grp(1, "Group 1", &[1]), grp(10, "Group 2", &[2, 3])]);
    }

    #[test]
    fn grouping_a_groups_first_effect_goes_before_what_stays() {
        let l = vec![fx(0), grp(1, "Group 1", &[1, 2])];
        let out = group(&l, &[1, 0], 10, None).unwrap();
        // 0 is first, top level: the new group takes its place, 1 leaves group 1.
        assert_eq!(out, [grp(10, "Group 2", &[0, 1]), grp(1, "Group 1", &[2])]);
        let out = group(&l, &[1], 10, None).unwrap();
        assert_eq!(
            out,
            [fx(0), grp(10, "Group 2", &[1]), grp(1, "Group 1", &[2])]
        );
    }

    #[test]
    fn regrouping_a_whole_group_reuses_its_name() {
        let l = vec![grp(1, "Group 1", &[1, 2]), fx(3)];
        let out = group(&l, &[1, 2, 3], 10, None).unwrap();
        assert_eq!(out, [grp(10, "Group 1", &[1, 2, 3])]);
    }

    #[test]
    fn grouping_needs_effects_in_the_chain() {
        let l = vec![fx(1)];
        assert!(group(&l, &[], 10, None).is_err());
        assert!(group(&l, &[9], 10, None).is_err());
    }

    #[test]
    fn names_take_the_lowest_free_number() {
        let l = vec![grp(1, "Group 2", &[1]), grp(2, "Drums", &[2])];
        assert_eq!(next_name(&l), "Group 1");
        let l = vec![grp(1, "Group 1", &[1]), grp(2, "Group 3", &[2])];
        assert_eq!(next_name(&l), "Group 2");
    }

    #[test]
    fn ungroups_in_place() {
        let l = vec![fx(1), grp(10, "G", &[2, 3]), fx(4)];
        assert_eq!(ungroup(&l, 10).unwrap(), [fx(1), fx(2), fx(3), fx(4)]);
        assert!(ungroup(&l, 99).is_err());
    }

    #[test]
    fn removes_a_group_with_its_effects() {
        let l = vec![fx(1), grp(10, "G", &[2, 3])];
        assert_eq!(remove_group(&l, 10).unwrap(), [fx(1)]);
    }

    #[test]
    fn moves_a_group_as_a_block() {
        let l = vec![fx(1), grp(10, "G", &[2, 3]), fx(4)];
        let out = move_group(&l, 10, 0).unwrap();
        assert_eq!(out, [grp(10, "G", &[2, 3]), fx(1), fx(4)]);
        assert_eq!(
            move_group(&l, 10, 99).unwrap().last(),
            Some(&grp(10, "G", &[2, 3]))
        );
    }

    #[test]
    fn moves_effects_into_and_out_of_groups() {
        let l = vec![grp(10, "G", &[1, 2]), fx(3)];
        let into = Target {
            group: Some(10),
            index: 1,
        };
        assert_eq!(
            move_effect(&l, 3, into).unwrap(),
            [grp(10, "G", &[1, 3, 2])]
        );
        let out = Target {
            group: None,
            index: 1,
        };
        assert_eq!(
            move_effect(&l, 2, out).unwrap(),
            [grp(10, "G", &[1]), fx(2), fx(3)]
        );
        // Within a group, the index counts after the effect is taken out.
        let first = Target {
            group: Some(10),
            index: 0,
        };
        assert_eq!(
            move_effect(&l, 2, first).unwrap(),
            [grp(10, "G", &[2, 1]), fx(3)]
        );
    }

    #[test]
    fn moving_the_last_effect_out_removes_the_group() {
        let l = vec![grp(10, "G", &[1]), fx(2)];
        let to = Target {
            group: None,
            index: 2,
        };
        assert_eq!(move_effect(&l, 1, to).unwrap(), [fx(2), fx(1)]);
        // Into its own group, it stays.
        let back = Target {
            group: Some(10),
            index: 0,
        };
        assert_eq!(move_effect(&l, 1, back).unwrap(), l);
    }

    #[test]
    fn moving_to_a_missing_group_fails() {
        let l = vec![fx(1)];
        let to = Target {
            group: Some(5),
            index: 0,
        };
        assert!(move_effect(&l, 1, to).is_err());
        assert!(
            move_effect(
                &l,
                9,
                Target {
                    group: None,
                    index: 0
                }
            )
            .is_err()
        );
    }

    #[test]
    fn removing_the_last_effect_removes_its_group() {
        let l = vec![grp(10, "G", &[1]), fx(2)];
        assert_eq!(remove_effect(&l, 1).unwrap(), [fx(2)]);
        assert!(remove_effect(&l, 9).is_err());
    }

    #[test]
    fn renames_with_a_trimmed_name() {
        let l = vec![grp(10, "G", &[1])];
        assert_eq!(
            rename_group(&l, 10, "  Vocal cleanup ").unwrap(),
            [grp(10, "Vocal cleanup", &[1])]
        );
        assert!(rename_group(&l, 10, "   ").is_err());
        assert!(rename_group(&l, 10, &"x".repeat(65)).is_err());
        assert!(rename_group(&l, 10, &"é".repeat(64)).is_ok());
    }
}
