use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::ir::*;

pub(crate) fn make_regex(r: &str) -> Result<regex::Regex, regex::Error> {
    regex::Regex::new(&format!("^{}$", r))
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Serialize, Deserialize)]
pub enum CheckLevel {
    NoCheck,
    Layout,
    Names,
    Descriptions,
}

pub(crate) fn check_mergeable_enums(a: &Enum, b: &Enum, level: CheckLevel) -> anyhow::Result<()> {
    if let Err(e) = check_mergeable_enums_inner(a, b, level) {
        bail!(
            "Cannot merge enums.\nfirst: {:#?}\nsecond: {:#?}\ncause: {:?}",
            a,
            b,
            e
        )
    }
    Ok(())
}
pub(crate) fn check_mergeable_enums_inner(
    a: &Enum,
    b: &Enum,
    level: CheckLevel,
) -> anyhow::Result<()> {
    if a.bit_size != b.bit_size {
        bail!("Different bit size: {} vs {}", a.bit_size, b.bit_size)
    }

    if level >= CheckLevel::Layout {
        if a.variants.len() != b.variants.len() {
            bail!("Different variant count")
        }

        let mut aok = [false; 1024];
        let mut bok = [false; 1024];

        for (ia, fa) in a.variants.iter().enumerate() {
            if let Some((ib, _fb)) = b
                .variants
                .iter()
                .enumerate()
                .find(|(ib, fb)| !bok[*ib] && mergeable_variants(fa, fb, level))
            {
                aok[ia] = true;
                bok[ib] = true;
            } else {
                bail!("Variant in first enum has no match: {:?}", fa);
            }
        }
    }

    Ok(())
}

pub(crate) fn mergeable_variants(a: &EnumVariant, b: &EnumVariant, level: CheckLevel) -> bool {
    let mut res = true;
    if level >= CheckLevel::Layout {
        res &= a.value == b.value;
    }
    if level >= CheckLevel::Names {
        res &= a.name == b.name;
    }
    if level >= CheckLevel::Descriptions {
        res &= a.description == b.description;
    }
    res
}

impl Default for CheckLevel {
    fn default() -> Self {
        Self::Names
    }
}

pub(crate) fn check_mergeable_fieldsets(
    a: &FieldSet,
    b: &FieldSet,
    level: CheckLevel,
) -> anyhow::Result<()> {
    if let Err(e) = check_mergeable_fieldsets_inner(a, b, level) {
        bail!(
            "Cannot merge fieldsets.\nfirst: {:#?}\nsecond: {:#?}\ncause: {:?}",
            a,
            b,
            e
        )
    }
    Ok(())
}

pub(crate) fn mergeable_fields(a: &Field, b: &Field, level: CheckLevel) -> bool {
    let mut res = true;
    if level >= CheckLevel::Layout {
        res &= a.bit_size == b.bit_size
            && a.bit_offset == b.bit_offset
            && a.enumm == b.enumm
            && a.array == b.array;
    }
    if level >= CheckLevel::Names {
        res &= a.name == b.name;
    }
    if level >= CheckLevel::Descriptions {
        res &= a.description == b.description;
    }
    res
}

pub(crate) fn check_mergeable_fieldsets_inner(
    a: &FieldSet,
    b: &FieldSet,
    level: CheckLevel,
) -> anyhow::Result<()> {
    if a.bit_size != b.bit_size {
        bail!("Different bit size: {} vs {}", a.bit_size, b.bit_size)
    }

    if level >= CheckLevel::Layout {
        if a.fields.len() != b.fields.len() {
            bail!("Different field count")
        }

        let mut aok = [false; 128];
        let mut bok = [false; 128];

        for (ia, fa) in a.fields.iter().enumerate() {
            if let Some((ib, _fb)) = b
                .fields
                .iter()
                .enumerate()
                .find(|(ib, fb)| !bok[*ib] && mergeable_fields(fa, fb, level))
            {
                aok[ia] = true;
                bok[ib] = true;
            } else {
                bail!("Field in first fieldset has no match: {:?}", fa);
            }
        }
    }

    Ok(())
}

pub(crate) fn match_all(set: impl Iterator<Item = String>, re: &regex::Regex) -> HashSet<String> {
    let mut ids: HashSet<String> = HashSet::new();
    for id in set {
        if re.is_match(&id) {
            ids.insert(id);
        }
    }
    ids
}

pub(crate) fn match_groups(
    set: impl Iterator<Item = String>,
    re: &regex::Regex,
    to: &str,
) -> HashMap<String, HashSet<String>> {
    let mut groups: HashMap<String, HashSet<String>> = HashMap::new();
    for s in set {
        if let Some(to) = match_expand(&s, re, to) {
            if let Some(v) = groups.get_mut(&to) {
                v.insert(s);
            } else {
                let mut v = HashSet::new();
                v.insert(s);
                groups.insert(to, v);
            }
        }
    }
    groups
}

pub(crate) fn match_expand(s: &str, regex: &regex::Regex, res: &str) -> Option<String> {
    let m = regex.captures(s)?;
    let mut dst = String::new();
    m.expand(res, &mut dst);
    Some(dst)
}

pub(crate) fn replace_enum_ids(ir: &mut IR, from: &HashSet<String>, to: String) {
    for (_, fs) in ir.fieldsets.iter_mut() {
        for f in fs.fields.iter_mut() {
            if let Some(id) = &mut f.enumm {
                if from.contains(id) {
                    *id = to.clone()
                }
            }
        }
    }
}

pub(crate) fn replace_fieldset_ids(ir: &mut IR, from: &HashSet<String>, to: String) {
    for (_, b) in ir.blocks.iter_mut() {
        for i in b.items.iter_mut() {
            if let BlockItemInner::Register(r) = &mut i.inner {
                if let Some(id) = &r.fieldset {
                    if from.contains(id) {
                        r.fieldset = Some(to.clone())
                    }
                }
            }
        }
    }
}

pub(crate) fn replace_block_ids(ir: &mut IR, from: &HashSet<String>, to: String) {
    for (_, d) in ir.devices.iter_mut() {
        for p in d.peripherals.iter_mut() {
            if let Some(block) = &mut p.block {
                if from.contains(block) {
                    *block = to.clone()
                }
            }
        }
    }

    for (_, b) in ir.blocks.iter_mut() {
        for i in b.items.iter_mut() {
            if let BlockItemInner::Block(bi) = &mut i.inner {
                if from.contains(&bi.block) {
                    bi.block = to.clone()
                }
            }
        }
    }
}

pub(crate) fn calc_array(mut offsets: Vec<u32>) -> (u32, Array) {
    offsets.sort_unstable();

    // Guess stride.
    let start_offset = offsets[0];
    let len = offsets.len() as u32;
    let stride = if len == 1 {
        // If there's only 1 item, we can't know the stride, but it
        // doesn't really matter!
        0
    } else {
        offsets[1] - offsets[0]
    };

    // Check the stride guess is OK

    if offsets
        .iter()
        .enumerate()
        .all(|(n, &i)| i == start_offset + (n as u32) * stride)
    {
        // Array is regular,
        (
            start_offset,
            Array::Regular(RegularArray {
                len: offsets.len() as _,
                stride,
            }),
        )
    } else {
        // Array is irregular,
        for o in &mut offsets {
            *o -= start_offset
        }
        (start_offset, Array::Cursed(CursedArray { offsets }))
    }
}

// filter enum by enum name, then copy variant description
pub(crate) fn extract_variant_desc(
    ir: &IR,
    enum_names: &str,
    bit_size: Option<u32>,
) -> anyhow::Result<HashMap<String, String>> {
    let re = make_regex(enum_names)?;

    let mut enum_desc_pair: HashMap<String, String> = HashMap::new();
    for (e_name, e_struct) in ir.enums.iter().filter(|(e_name, e_struct)| {
        bit_size.map_or(true, |s| s == e_struct.bit_size) && re.is_match(e_name)
    }) {
        let variant_desc_str = e_struct.variants.iter().fold(String::new(), |mut acc, v| {
            acc.push_str(
                format!(
                    "{}: {}\n",
                    v.value,
                    v.description.clone().unwrap_or_default()
                )
                .as_str(),
            );
            acc
        });

        enum_desc_pair.insert(e_name.clone(), variant_desc_str);
    }

    Ok(enum_desc_pair)
}

// filter field by enum name, then append corresponding variant description
pub(crate) fn append_variant_desc_to_field(
    ir: &mut IR,
    enum_desc_pair: &HashMap<String, String>,
    bit_size: Option<u32>,
) {
    for fs in ir.fieldsets.values_mut() {
        for f in fs
            .fields
            .iter_mut()
            .filter(|f| bit_size.map_or(true, |s| s == f.bit_size) && f.enumm.is_some())
        {
            for (_, desc_string) in enum_desc_pair
                .iter()
                .filter(|(e_name, _)| **e_name == f.enumm.clone().unwrap())
            {
                match &f.description {
                    Some(desc) => {
                        f.description = Some(format!("{}\n{}", desc.clone(), desc_string.clone()))
                    }
                    None => f.description = Some(desc_string.clone()),
                }
            }
        }
    }
}
