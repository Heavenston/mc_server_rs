use mc_networking::packets::client_bound::{ 
    C23RegistryCodec,
    C23BiomeElement,
    C23BiomeEffects,
    C23DimensionElement,
};

lazy_static::lazy_static! {
    pub static ref REGISTRY_CODEC: C23RegistryCodec = C23RegistryCodec {
        biomes: vec![
            ("heav:your_void".into(), C23BiomeElement {
                precipitation: "none".into(),
                temperature: 0.5,
                temperature_modifier: None,
                downfall: 0.5,
                category: "none".into(),
                depth: 0.,
                scale: 1.,
                effects: C23BiomeEffects {
                    water_color: 4159204,
                    music: None,
                    mood_sound: None,
                    additions_sound: None,
                    ambient_sound: None,
                    water_fog_color: 329011,
                    fog_color: 12638463,
                    sky_color: 8103167,

                    folliage_color: None,
                    grass_color: None,
                    grass_color_modifier: None,
                    particle: None,
                },
            })
        ],
        dimension_types: vec![
            ("heav:voidy".into(), C23DimensionElement {
                shrunk: 0,
                ultrawarm: 0,
                infiniburn: "#minecraft:infiniburn_overworld".into(),
                piglin_safe: 0,
                ambient_light: 0.,
                has_skylight: 1,
                has_ceiling: 0,
                effects: "minecraft:overworld".into(),
                has_raids: 1,
                monster_spawn_block_light_limit: 0,
                respawn_anchor_works: 0,
                min_y: 0,
                logical_height: 126,
                height: 256,
                monster_spawn_light_level: 7,
                natural: 1,
                bed_works: 1,
                coordinate_scale: 1.,
                fixed_time: None,
            })
        ],
        chat_types: (),
    };
}
