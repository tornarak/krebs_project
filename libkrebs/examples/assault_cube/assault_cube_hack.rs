//! The "Hello, World!" of game hacking.
//!
//! Demonstrates basic functionality.

#![allow(dead_code)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use std::fmt;
use std::thread;
use std::time::Duration;

// use winapi::um::winuser::*;

use libkrebs::mem::{MemAddress, Reader, Writer};
use libkrebs::NativeProcess;

/* Constants */

#[cfg(windows)]
const player_obj_ptr: MemAddress = 0x0050F4F4;
#[cfg(unix)]
const player_obj_ptr: MemAddress = 0x005A3518;

#[cfg(windows)]
const hp_offset: MemAddress = 0xF8;
#[cfg(unix)]
const hp_offset: MemAddress = 0x100;

// TODO: verify on Linux
const can_jump_offset: MemAddress = 0x69;
const velocity_offset: MemAddress = 0x28;

// Windows: 9 gun slots × 4 bytes = 0x24; array starts at 0x350 → current-gun-ptr at 0x374
// Linux:   9 gun slots × 8 bytes = 0x48; array starts at 0x350 → current-gun-ptr at 0x398
#[cfg(windows)]
const gun_offset: MemAddress = 0x374;
#[cfg(unix)]
const gun_offset: MemAddress = 0x398;

// Offset within gun struct to the reserve/magazine ammo pointers.
// On Linux all four pointer fields before these grew 4→8 bytes (+0x10 total shift).
#[cfg(windows)]
const reserve_offset: MemAddress = 0x10;
#[cfg(unix)]
const reserve_offset: MemAddress = 0x20;

#[cfg(windows)]
const magazine_offset: MemAddress = 0x14;
#[cfg(unix)]
const magazine_offset: MemAddress = 0x28;

// The ammo pointer in the gun struct points directly at the int32 on both platforms.
const ammo_value_offset: MemAddress = 0x00;

// Note : Reserve struct is right behind mag struct
// Setting about 0x50 or so bytes after reserve to high value
//  gives instant everything

#[derive(Copy, Clone, Debug)]
enum WeaponOffsets {
    PISTOL = 0x00,
    CARBINE = 0x04,
    SHOTGUN = 0x08,
    SMG = 0x0C,
    SNIPER = 0x10,
    MACHINE_GUN = 0x14,
    GRENADE = 0x1C,
    DUAL_PISTOL = 0x20,
}

impl fmt::Display for WeaponOffsets {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

//	let const time_to_shot_offset : Vec<u32> = vec![0x28]

/* Vector 3 */

#[repr(C)]
#[derive(Copy, Clone)]
struct Vector3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vector3 {
    fn distance(&self, other: &Self) -> f32 {
        f32::sqrt(
            (self.x - other.x) * (self.x - other.x)
                + (self.y - other.y) * (self.y - other.y)
                + (self.z - other.z) * (self.z - other.z),
        )
    }
}

impl PartialEq for Vector3 {
    fn eq(&self, other: &Self) -> bool {
        self.distance(other) <= 5.0
    }
}

impl Eq for Vector3 {}

impl fmt::Display for Vector3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

/* Game State Repr. */

#[repr(C)]
#[derive(Copy, Clone)]
struct Ammo {
    pistol: u32,
    carbine: u32,
    shotgun: u32,
    smg: u32,
    sniper: u32,
    machine_gun: u32,
    grenade: u32,
    dual_pistol: u32,
}

struct GamePointers {
    player_obj_addr: MemAddress,

    hp_addr: MemAddress,
    can_jump_addr: MemAddress,
    velocity_addr: MemAddress,

    reserve_addr: MemAddress,
    magazine_addr: MemAddress,
}

struct GameState {
    hp: i32,
    can_jump: bool,
    velocity: Vector3,

    reserve: Ammo,
    magazine: Ammo,
}

impl PartialEq for GameState {
    fn eq(&self, other: &Self) -> bool {
        self.hp == other.hp && self.can_jump == other.can_jump && self.velocity == other.velocity
    }
}

impl fmt::Display for GameState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HP\t\t\t{}\nCan Jump:\t{}\nVelocity:\t{}\nPistol Ammo:\t{}",
            self.hp, self.can_jump, self.velocity, self.magazine.pistol
        )
    }
}

impl GamePointers {
    pub unsafe fn new(game_proc: &mut NativeProcess) -> GamePointers {
        println!("Getting Player Object");
        let player_obj_addr: MemAddress = game_proc.read_to_type(player_obj_ptr).unwrap();
        let hp_addr: MemAddress = game_proc
            .deref_chain(player_obj_addr, &vec![hp_offset])
            .unwrap();
        let can_jump_addr: MemAddress = game_proc
            .deref_chain(player_obj_addr, &vec![can_jump_offset])
            .unwrap();
        let velocity_addr: MemAddress = game_proc
            .deref_chain(player_obj_addr, &vec![velocity_offset])
            .unwrap();

        let reserve_addr: MemAddress = game_proc
            .deref_chain(player_obj_addr, &vec![gun_offset, reserve_offset, ammo_value_offset])
            .unwrap();
        let magazine_addr: MemAddress = game_proc
            .deref_chain(player_obj_addr, &vec![gun_offset, magazine_offset, ammo_value_offset])
            .unwrap();

        println!("Getting HP, Ammo, Etc...");
        GamePointers {
            player_obj_addr,

            hp_addr,
            can_jump_addr,
            velocity_addr,

            reserve_addr,
            magazine_addr,
        }
    }
}

struct GameHack {
    game_proc: NativeProcess,
    pointers: GamePointers,
}

impl GameHack {
    pub unsafe fn new(mut game_proc: NativeProcess) -> GameHack {
        println!("Initializing game");
        let pointers = GamePointers::new(&mut game_proc);

        GameHack {
            game_proc,
            pointers,
        }
    }

    pub fn get_state(&mut self) -> GameState {
        GameState {
            hp: self.game_proc.read_to_type(self.pointers.hp_addr).unwrap(),
            can_jump: self
                .game_proc
                .read_to_type(self.pointers.can_jump_addr)
                .unwrap(),
            velocity: self
                .game_proc
                .read_to_type(self.pointers.velocity_addr)
                .unwrap(),

            reserve: self
                .game_proc
                .read_to_type(self.pointers.reserve_addr)
                .unwrap(),
            magazine: self
                .game_proc
                .read_to_type(self.pointers.magazine_addr)
                .unwrap(),
        }
    }

    // TODO
    pub fn set_state(&mut self, state: GameState) -> Result<(), String> {
        self.game_proc
            .write_from_type(self.pointers.hp_addr, state.hp)
            .unwrap();
        self.game_proc
            .write_from_type(self.pointers.can_jump_addr, state.can_jump)
            .unwrap();
        self.game_proc
            .write_from_type(self.pointers.velocity_addr, state.velocity)
            .unwrap();

        self.game_proc
            .write_from_type(self.pointers.reserve_addr, state.reserve)
            .unwrap();
        self.game_proc
            .write_from_type(self.pointers.magazine_addr, state.magazine)
            .unwrap();

        Ok(())
    }
}

pub fn hax_game(game_proc: NativeProcess) {
    let mut hack = unsafe { GameHack::new(game_proc) };
    let mut last_state = hack.get_state();

    loop {
        thread::sleep(Duration::from_millis(100));

        let state = hack.get_state();

        if state != last_state {
            println!("{}", state);
            last_state = state;
        }

        #[cfg(windows)]
        if libkrebs::win::misc::get_async_key_state(winapi::um::winuser::VK_F3) {
            break;
        }
    }
}
