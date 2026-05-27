"""OSRS-style post-processing helpers."""

from stonepyre_asset_forge.config import StyleConfig


OSRS_CHARACTER = StyleConfig(
    target_tris=1200,
    flat_shading=True,
    texture_size=256,
    simplify_materials=True,
    normalize_scale=True,
    center_origin=True,
    normalize_height=1.8,
)

OSRS_CREATURE = StyleConfig(
    target_tris=1000,
    flat_shading=True,
    texture_size=256,
    simplify_materials=True,
    normalize_scale=True,
    center_origin=True,
    normalize_height=1.5,
)

OSRS_PROP = StyleConfig(
    target_tris=800,
    flat_shading=True,
    texture_size=128,
    simplify_materials=True,
    normalize_scale=True,
    center_origin=True,
    normalize_height=1.0,
)

OSRS_TREE = StyleConfig(
    target_tris=600,
    flat_shading=True,
    texture_size=128,
    simplify_materials=True,
    normalize_scale=True,
    center_origin=True,
    normalize_height=3.0,
)

OSRS_BUILDING = StyleConfig(
    target_tris=1500,
    flat_shading=True,
    texture_size=256,
    simplify_materials=True,
    normalize_scale=True,
    center_origin=True,
    normalize_height=4.0,
)
