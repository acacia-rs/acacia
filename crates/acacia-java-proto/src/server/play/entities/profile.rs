//! `ResolvableProfile.STREAM_CODEC`: the player-head profile entity data carries (mannequins, skulls).

use super::Optional;
use crate::server::login::{GameProfile, ProfileProperty};
use crate::{Error, Wire};

/// `ResolvableProfile.Partial`: what the client resolves into a full profile.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PartialProfile {
    pub name: Optional<String>,
    pub id: Optional<u128>,
    pub properties: Vec<ProfileProperty>,
}

impl Wire for PartialProfile {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { name: Optional::read(input)?, id: Optional::read(input)?, properties: Vec::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.name.write(out);
        self.id.write(out);
        self.properties.write(out);
    }
}

/// `ByteBufCodecs.either(GAME_PROFILE, Partial)`: a flag, true for the full profile.
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileSource {
    Full(GameProfile),
    Partial(PartialProfile),
}

impl Default for ProfileSource {
    fn default() -> Self {
        Self::Partial(PartialProfile::default())
    }
}

impl Wire for ProfileSource {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        if bool::read(input)? { GameProfile::read(input).map(Self::Full) } else { PartialProfile::read(input).map(Self::Partial) }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Full(profile) => {
                true.write(out);
                profile.write(out);
            }
            Self::Partial(partial) => {
                false.write(out);
                partial.write(out);
            }
        }
    }
}

/// `PlayerSkin.Patch`: texture identifiers overriding the profile's, and the model (`slim` true, wide false).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SkinPatch {
    pub body: Optional<String>,
    pub cape: Optional<String>,
    pub elytra: Optional<String>,
    pub slim: Optional<bool>,
}

impl Wire for SkinPatch {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { body: Optional::read(input)?, cape: Optional::read(input)?, elytra: Optional::read(input)?, slim: Optional::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.body.write(out);
        self.cape.write(out);
        self.elytra.write(out);
        self.slim.write(out);
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResolvableProfile {
    pub source: ProfileSource,
    pub skin: SkinPatch,
}

impl Wire for ResolvableProfile {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { source: ProfileSource::read(input)?, skin: SkinPatch::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.source.write(out);
        self.skin.write(out);
    }
}
