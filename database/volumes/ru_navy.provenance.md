# ru_navy.json — provenance

Sources (page numbers are the printed folios, not PDF page numbers):

- **RN** = *Russia's Navy*, Admiralty Trilogy, 1 Dec 25 digital update
  (file `9-4.Russias_Navy_1_Dec_25.pdf`, 112 PDF pages).
  Annex A ships use folios `A-n`; ship systems use `C-1` (guns), `D-1..D-2`
  (surface missiles), `E-1` (ASW projectors/standoff), `F-1..F-2`
  (torpedoes), `J-1..J-4` (radars), `K-1..K-3` (sonars).
- **RA** = *Russia's Aircraft*, Admiralty Trilogy, 1 Dec 25 digital update
  (file `8-4.Russias_Aircraft_1_Dec_25.pdf`, 92 PDF pages). Aircraft use
  folios `B-n`; weapons `H2-3`, `H4-1..H4-2`.

Page text was read from the project RAG database
`鱼叉/resources/rag/harpoon_rules.sqlite3` (`chunks` rows for document
`source_804dec425322` = RN and `source_abc1d7408e61` = RA), which is the
per-page extraction of the two PDFs. The cited folio is the header printed
on that page (e.g. RN PDF page 33 begins `A-30`).

## Platforms (55)

| id | class | source | page | data taken / notes |
|---|---|---|---|---|
| ru_kuznetsov | Pr. 1143.5 Admiral Kuznetsov CV | RN | A-4 | DP 1110, 29 kt, Large/Loud; 12 P-700, 4x Kinzhal (48), 8x Kortik, 6x AK-630, 2x RBU 12000; air wing 14 Su-33 / 10 MiG-29KR / 11 Ka-27 / 2 Ka-31R |
| ru_gorshkov_cv | Pr. 1143.4 Admiral Gorshkov CVHG | RN | A-4 | DP 790, 30 kt, Large/Noisy; 12 P-500, 2x Shtorm-M (48), 2x Kinzhal (8), 2x AK-726, 8x AK-630; helicopter wing |
| ru_novorossiysk | Pr. 1143.3 Novorossiysk CVHG | RN | A-5 | DP 772, 32 kt, Large/Noisy; Kinzhal not fitted per entry and therefore omitted |
| ru_kiev | Pr. 1143 Kiev CVHG | RN | A-5 | DP 749, 32 kt, Large/Noisy; 8 P-500, 2x Shtorm-M, 2x Osa-M (20), 533 mm TT |
| ru_moskva | Pr. 1123 Moskva CHG | RN | A-5 | DP 400, 28 kt, Med/Noisy; 2x Shtorm (48), 2x AK-725 (modelled with gun_76mm), RBU, TT |
| ru_kirov | Pr. 1144 Kirov CGN | RN | A-25 | DP 640, 31 kt, Large/Noisy; 20 P-700, 12x S-300F (96), 2x Osa-M (20), 2x AK-100, 8x AK-630, Metel, 533 mm TT |
| ru_pyotr_velikiy | Pr. 1144.2 Pyotr Velikiy CGN | RN | A-24 | DP 649, 31 kt; 20 P-700, 12x S-300FM (96), 2x Kinzhal (32), 6x Kortik, AK-130, TT |
| ru_nakhimov | Pr. 1144M Admiral Nakhimov CGN | RN | A-23 | DP 644, 31 kt; 80 UKSK cells (64 Oniks + 16 Kalibr), 12x S-300FM (96), AK-192M, 6x Pantsir-M, Paket-NK |
| ru_slava | Pr. 1164 Atlant CG | RN | A-25 | DP 284, 32 kt, Med/Noisy; 16 P-500, 8x S-300F (64), 2x Osa-MA (20), AK-130, 6x AK-630, TT |
| ru_varyag | Pr. 1164.1 Atlant CG | RN | A-24 | DP 350, 32 kt; 16 P-1000, 8x S-300F (64), AK-130, 5x Kortik, TT |
| ru_kerch | Pr. 1134B Berkut B Kara CG | RN | A-25/A-26 | DP 272, 32 kt; 2x Shtorm (80), 2x Osa-M (20), 2x Metel, 2x AK-726, 4x AK-630, TT |
| ru_sovremenny | Pr. 956 Sarych Sovremenny DDG | RN | A-30 | DP 223, 32 kt, Med/Noisy; 8 Moskit, 2x Uragan (48), 2x AK-130, 4x AK-630, 2x RBU 1000, 4 TT |
| ru_sovremenny_a | Pr. 956A Mod Sovremenny DDG | RN | A-30 | DP 223, 32 kt; same fit but Moskit-M (65 nm) |
| ru_udaloy | Pr. 1155 Udaloy DD | RN | A-32 | DP 232, 30 kt, Med/Quiet; 2x Kinzhal (32), 2x Metel, 2x AK-100, 4x AK-630, 2x RBU 6000, 4 TT |
| ru_chabanenko | Pr. 1155.1 Udaloy II DD | RN | A-32 | DP 249, 30 kt; 8 Moskit-M, 2x Kinzhal (32), AK-130, 2x Kortik-M, 2x RBU 12000, 8 TT |
| ru_gorshkov | Pr. 22350 Admiral Gorshkov FFG | RN | A-36 | DP 209, 30 kt, VSmall/Quiet; 32 Redut, 2x 8-cell UKSK (8 Oniks + 8 Kalibr), A-192M, 2x Palash, Paket-NK |
| ru_22350m | Pr. 22350M FFG | RN | A-36 | DP 209, 29 kt; 32 Redut, 24 UKSK (8 Oniks + 8 Kalibr + 8 Tsirkon), A-192M, 2x Palash, Paket-NK; future/estimated fit |
| ru_grigorovich | Pr. 1135.6R Grigorovich FFG | RN | A-36 | DP 201, 30 kt, VSmall/Quiet; 24 Shtil-1, 8 UKSK Kalibr, A-190, 2x AK-630, RPK-8, 4 TT |
| ru_neustrashimy | Pr. 1154 Neustrashimy FF | RN | A-37 | DP 181, 30 kt, Small/Quiet; 32 Kinzhal, 2x Kortik (64), AK-100, RBU-6000, 12 TT weapons |
| ru_krivak3 | Pr. 1135.1 Nerey Krivak III FF | RN | A-37 | DP 138, 30 kt, Small/Noisy; 20 Osa-MA, AK-100, 2x AK-630, 2x RBU 6000, 4 TT |
| ru_gepard2 | Pr. 1166.1K Gepard II FFL | RN | A-40 | DP 95, 28 kt, Small/Noisy; 8 UKSK Oniks, AK-176, Palash, helo pad only (no hangar) |
| ru_steregushchy | Pr. 20380 Steregushchiy FFL | RN | A-40 | DP 116, 26 kt, VSmall/Quiet; 8 Uran, A-190, Kortik-M (32), 2x AK-630, Paket-NK |
| ru_soobrazitelny | Pr. 20381 Steregushchiy II FFL | RN | A-40 | DP 116, 26 kt; 16 Redut, 8 Uran, A-190, 2x AK-630, Paket-NK |
| ru_gremyashchy | Pr. 20385 Gremyashchy FFL | RN | A-39 | DP 130, 27 kt, VSmall/Noisy; 16 Redut, 8 UKSK Kalibr, A-190, 2x AK-630, Paket-NK |
| ru_karakurt | Pr. 22800 Karakurt FFL | RN | A-39 | DP 69, 30 kt, VSmall/Noisy; 8 UKSK Kalibr/Oniks, AK-176MA, Pantsir-M (late units; first two had AK-630) |
| ru_buyan_m | Pr. 21631 Buyan-M FFL | RN | A-39 | DP 66, 26 kt; 8 UKSK Kalibr/Oniks, A-190M, AK-630M-2 Duet, 8 Igla-1M |
| ru_tarantul4 | Pr. 1241.8 Tarantul IV PTG | RN | A-50 | DP 37, 29 kt, Small/Noisy; 8 Uran, AK-176MA, 2x AK-630M |
| ru_tarantul3 | Pr. 1241.1M Tarantul III PTG | RN | A-50 | DP 37, 38 kt; 4 Moskit-M, AK-176, 2x AK-630, Igla |
| ru_tarantul2 | Pr. 1241.1T Tarantul II PTG | RN | A-51 | DP 34, 42 kt; 4 Termit-M (modelled as P-120, same 65 nm class), AK-176, 2x AK-630, Igla |
| ru_nanuchka3 | Pr. 1234.1 Nanuchka III PGG | RN | A-48 | DP 47, 32 kt, Small/Noisy; 6 P-120, Osa-M (20), AK-176, AK-630 |
| ru_nanuchka4 | Pr. 1234.7 Nanuchka IV PGG | RN | A-48 | DP 47, 34 kt; 12 Oniks (test ship Nakat), Osa-M (20), AK-176, AK-630 |
| ru_dergach | Pr. 1239 Sivuch Dergach PGGA | RN | A-49 | DP 36, 53 kt, Small/VQuiet; 8 Moskit-M, Osa-MA (20), AK-176, 2x AK-630, Igla |
| ru_borei | Pr. 955 Borei SSBN | RN | A-6 | DP 308, 29 kt subm, Medium/VQuiet; 16 R-30 Bulava tube **omitted (no SLBM weapon kind)**; 28 torpedo weapons |
| ru_borei_2 | Pr. 955A Borei II SSBN | RN | A-6 | DP 320, 29 kt; Bulava omitted; 28 torpedo weapons |
| ru_yasen | Pr. 885 Yasen SSGN | RN | A-11 | DP 221, 31 kt, Small/EQuiet; 32 VLS (24 Oniks + 8 Kalibr), 10 TT |
| ru_yasen_m | Pr. 885M Yasen-M SSGN | RN | A-10 | DP 217, 31 kt; 32 VLS, 10 TT |
| ru_oscar2 | Pr. 949A Antey Oscar II SSGN | RN | A-11/A-12 | DP 368, 31 kt, Medium/VQuiet; 24 P-700, 4x 533 mm + 2x 650 mm TT |
| ru_akula2 | Pr. 971M Akula II SSN | RN | A-16 | DP 211, 33 kt, Small/EQuiet; 4x 533 mm + 4x 650 mm TT |
| ru_akula1 | Pr. 971 Akula I SSN | RN | A-16 | DP 204, 33 kt, Small/VQuiet; 4x 533 mm + 4x 650 mm TT |
| ru_sierra2 | Pr. 945A Kondor Sierra II SSN | RN | A-15 | DP 178, 33 kt, Small/VQuiet; 6x 533 mm TT |
| ru_delta4 | Pr. 667BDRM Del'fin Delta IV SSBN | RN | A-6/A-7 | DP 265, 24 kt, Small/Quiet; 16 R-29RM **omitted**; 4x 533 mm TT |
| ru_typhoon | Pr. 941 Akula Typhoon SSBN | RN | A-7 | DP 535, 25 kt, Medium/Quiet; 20 R-39 **omitted**; 6x 533 mm TT |
| ru_kilo | Pr. 636.3 Varshavyanka SS | RN | A-20 | DP 91, 20 kt, VSmall/VQuiet; 6x 533 mm TT, Klub-S capable |
| ru_kilo_636 | Pr. 636 Varshavyanka SS | RN | A-20 | DP 91, 20 kt; 6x 533 mm TT (No Klub fit in this variant) |
| ru_lada | Pr. 677 Lada SS | RN | A-19 | DP 70, 21 kt, VSmall/EQuiet; 6x 533 mm TT, Klub-S capable |
| ru_khabarovsk | Pr. 09851 Khabarovsk SSAN | RN | A-19 | DP 256, 29 kt, Small/EQuiet; 6x 533 mm TT (UGST-M; see substitutions) |
| ru_ivan_gren | Pr. 11711 Ivan Gren LST | RN | A-56 | DP 189, 18 kt; 1x AK-630M-2 + 2x AK-630M, Igla-S, 2x Ka-29TB |
| ru_ropucha | Pr. 775M Ropucha II LST | RN | A-56 | DP 126, 17 kt; AK-176, 2x AK-630, 2x Igla launchers |
| ru_alligator | Pr. 1171 Tapir Alligator LST | RN | A-57 | DP 113, 16 kt; 57 mm/25 mm mounts modelled with gun_76mm/ciws_30mm |
| ru_cv_su33 | Su-33 Flanker D | RA | B-42/B-43 | DV 35, cruise 485 kt, max 1240 kt, ceiling 17000 m, cruise range 1135 nmi; R-27ER/R-73/bombs |
| ru_cv_mig29kr | MiG-29KR Fulcrum D | RA | B-29/B-30 | DV 29, cruise 460 kt, max 1190 kt, ceiling 17500 m, range 715 nmi; R-77/R-73/Kh-35U |
| ru_cv_ka27 | Ka-27 Helix A ASW | RA | B-11 | DV 18, cruise 124 kt, max 146 kt, ceiling 5000 m, range 265 nmi; dipping sonar and torpedo |
| ru_cv_ka29 | Ka-29 Helix B Assault | RA | B-11/B-12 | DV 19, cruise 124 kt, max 150 kt, ceiling 4300 m, range 225 nmi; rockets/gun |
| ru_cv_ka31r | Ka-31R Helix E AEW | RA | B-12 | DV 17, cruise 100 kt, max 120 kt, ceiling 3500 m, range 255 nmi; E-801 radar |
| ru_cv_ka52k | Ka-52K Katran | RA | B-12/B-13 | DV 22, cruise 140 kt, max 160 kt, ceiling 5000 m, range 205 nmi; Kh-35U/Igla-V/rockets |

## Weapons (30)

All are new ids with the `ru_` prefix. Range/speed are the RN Annex D1
values (high-low flight path used for range where the annex prints two).
Damage is the annex's "Damage" column rounded to an engine-scale number;
where the column is a die expression (`70+D6`) the listed integer is the
mean rounded down (see estimates below).

| id | system | source | page | data |
|---|---|---|---|---|
| ru_p700 | P-700 Granit | RN | D-1 | 300 nm, 1620 kt, 70+D6 -> damage 90 (task-specified engine value) |
| ru_p1000 | P-1000 Vulkan | RN | D-2 | 378 nm, 1620 kt, 65+D6 -> 85 |
| ru_p500 | P-500 Bazalt | RN | D-1 | 300 nm, 1650 kt, 71+D6 -> 80 |
| ru_p270 | P-270 Moskit-M | RN | D-1 | 65 nm, 1650 kt, 52+D6 -> 58 |
| ru_p120 | P-120 Malakhit | RN | D-1 | 65 nm, 594 kt, 46+D6 -> 52 |
| ru_oniks | P-800 Oniks | RN | D-1 | 165 nm, 1650 kt, 42+D6/2 -> 49 |
| ru_kalibr | 3M54/3M54T Kalibr | RN | D-1 | 120 nm, 528 kt (terminal 1380), 42+D6/2 -> 49 |
| ru_uran | Kh-35 Uran | RN | D-2 | 65 nm, 580 kt, 35+D6/2 -> 41 |
| ru_tsirkon | 3M22 Tsirkon | RN | D-2 | 187 nm, 3150 kt, 47+D6 -> 53 |
| ru_kh35u | Kh-35U Uran | RA | H2-3 | 140.4 nm, 560 kt, 35+D6/2 -> 44 |
| ru_rastrub | 85RU Rastrub | RN | D-2, E-4 | 27 nm, 625 kt; modelled as AswRocket; ASuW secondary mode not modelled |
| ru_metel | 85R Metel | RN | E-4 | 27 nm, 625 kt; modelled as AswRocket |
| ru_65k | 65-76A Kit | RN | F-1 | 27 nm, 50 kt, 650 mm, 197 damage, wake-homing |
| ru_usp | USET-80K | RN | F-2 | 9.9 nm, 48 kt, 533 mm, 159 vs ship / 92 vs sub; single scalar = 159 |
| ru_fizik | Fizik-2 | RN | F-1 | 21.9 nm at 60 kt (27.4 at 40), 164 damage |
| ru_paket | Paket-NK MTT | RN | A-23 etc. | system listed but no RN annex row; launch range/damage estimated from MTT (F-2) |
| ru_s300f | S-300F Fort 5V55RD | RN | D-2 | 49.7 nm, 3885 kt, 39+D6 -> 45, 6 targets |
| ru_rif | S-300FM Fort-M 48N6/48N6DMK | RN | D-2 | 135 nm, 4080 kt, 39+D6 -> 48, 6 targets |
| ru_shtorm | Shtorm-M | RN | D-2 | 19 nm, 1650 kt, 33+D6/2 -> 33 |
| ru_uragan | Uragan 9M38 | RN | D-2 | 18.9 nm, 1650 kt, 28+D6/2 -> 31, 6 targets |
| ru_shtil | Shtil-1 9M317 | RN | D-2 | 27 nm, 2581 kt, 29+D6/2 -> 30 |
| ru_redut | Redut 9M96D | RN | D-2 | 64.8 nm, 1750 kt; 9M96 24 kg warhead scaled to 26 |
| ru_kinzhal | Kinzhal 9M330 | RN | D-1 | 6.5 nm, 1675 kt, 19+D6/3 -> 19 |
| ru_kortik | Kortik 9M311 | RN | D-1 | 5.4 nm, 1749 kt, 15+D6/3 -> 15; gun component folded into this entry |
| ru_pantsir | Pantsir-M 57E6 | RN | D-2 | 9.2 nm, 1979 kt, 19+D6/3 -> 19 |
| ru_osa | Osa-MA 9M33M2 | RN | D-1 | 6.5 nm, 1650 kt, 18+D6/3 -> 18 |
| ru_igla | Igla-S/Verba | RN | D-1 | 3.2 nm, 1166 kt, 2.5 warhead -> 12; IR |
| ru_r77 | R-77 Adder | RA | H4-2 | 36.7 nm, 1721 kt, 175 kg warhead -> 22 |
| ru_r73 | R-73 Archer | RA | H4-1 | 8.1 nm, 1434 kt, 105 kg -> 20; IR |
| ru_r27er | R-27ER Alamo C | RA | H4-1 | 39.9 nm, 2295 kt, 175 kg -> 22 |

## Sensors (6)

| id | system | source | page | data |
|---|---|---|---|---|
| ru_radar_fregat_m2 | MR-750 Fregat-M2 | RN | J-3 | 81 nm air-table / 25 nm surface-table, LAS |
| ru_radar_poliment | 5P20K Poliment | RN | J-1 | 81 / 27, LAS |
| ru_radar_furke4 | 5P27 Furke-4 | RN | J-1 | 108 / 30, LAS |
| ru_radar_podberezovik | MR-650 Podberezovik T1 | RN | J-3 | 270 / 25, LAS |
| ru_sonar_zarya2 | Zarya-2 | RN | K-3 | active 3.5 nm |
| ru_sonar_amfora | MGK-600 Irtysh-Amfora | RN | K-3 | passive 4.5 bow / 6.5 towed; single value 6.5 |

All generic sensor ids used by the platforms (`radar_air_xlong`,
`radar_air_med`, `radar_surf_med`, `radar_surf_sm`, `radar_nav`,
`esm_std`, `visual`, `sonar_p_hull`, `sonar_p_tow`, `sonar_a_hull`,
`sonar_dipping`) are the pre-existing built-ins; they were not redefined.

## Estimated / adapted values

The volume prints signature **classes** and damage-point progressions, not
engine dB, VPs, turn rates or countermeasure counts. The following are
adaptations, listed so no value is mistaken for a printed one:

1. **hp** = final (100%) number of the printed `Dam Pts` row. This is a
   direct transcription, not an estimate.
2. **max_speed_kts** = first (undamaged) printed `Surf Speed` / `Subm
   Speed` value. **cruise_speed_kts** estimated at roughly 60-65% of max
   for surface ships, 5-7 kt quiet patrol for submarines.
3. **noise_db** is a conversion of the printed acoustic class, anchored on
   the built-in scale documented in `rules/db_fleets.rs`: Loud 66, Noisy
   54-58, Quiet 40-46, VQuiet 28-34, EQuiet 22. Per-class values are
   interpolated within the band.
4. **rcs** uses the first printed signature word (`Large`, `Medium`,
   `Small`, `VSmall`).
5. **vp_value**, **turn_rate_deg_s**, **chaff**, **decoys**,
   **visual_factor** (subs 0.15) are engine-scale estimates from role and
   displacement; the volumes do not print them.
6. **endurance_s** for aircraft = printed cruise range / printed cruise
   speed, seconds. `ceiling` has no engine field and is recorded in the
   platform name.
7. **Aircraft RCS** from the printed Size/Signature line. Aircraft hp =
   printed Damage Value.
8. **P-700 damage 90** follows the work order's engine-scale value; the
   annex prints `70+D6` (mean ~73). This is the one weapon where the
   task-specified value deliberately overrides the book mean.
9. SAM damage is a linear engine-scale conversion of the annex warhead /
   damage column (e.g. S-300F 39+D6 -> 45, Redut 24 kg -> 26). Pk values
   are estimates within the built-in SAM bands (0.35-0.62).
10. **Rastrub/Metel** are guided ASW missiles with a secondary anti-ship
    mode. The engine's only standoff ASW kind is `AswRocket`, so they are
    fitted as such; their anti-ship mode is not modelled.
11. **Paket-NK** has no RN annex row; range 4 nm and damage 40 are
    estimated from the MTT lightweight torpedo (F-2, 4.9 nm) and marked
    estimated.
12. **USET-80K** prints different damage against ships (159) and
    submarines (92). `WeaponDef.damage` is scalar, so 159 is used and the
    anti-submarine reduction is not modelled.
13. **Aircraft ordnance**: FAB-500 is represented by the built-in
    `bomb_mk82` (same 0.8 nm overhead release, 30 damage); B-8 rocket
    pods by the built-in `rocket_57` (3 nm, salvo 4). Alternate loadouts
    are fitted simultaneously (Su-33 bombs + AAMs; MiG-29KR R-77 +
    Kh-35U) because the engine has no loadout selector.
14. **UGST-M** (Khabarovsk) has no RN Annex F row, so the boat is fitted
    with `ru_fizik` as the closest printed heavyweight electric torpedo;
    this is a substitution, not a transcription.
15. **RBU 6000 / RBU 12000 / UDAV-1** all use the built-in `asw_rocket`
    (6 nm, 25 damage). The volume's different ranges (6.3 / 3.2 nm) and
    warheads are collapsed into that one engine entry.
16. **AK-630 / Palash / AK-630M-2** use the built-in `ciws_30mm`; the
    76 mm mounts use `gun_76mm`, 100 mm `gun_100mm`, 130 mm
    `gun_130mm`, 57 mm/older mounts are approximated by `gun_76mm`.
17. **Kortik / Pantsir-M** combine gun and missile in one mount; each is
    modelled as one `Ciws` entry with its missile range and a single
    damage/Pk. The 30 mm gun component is not separately modelled.
18. **Karakurt** is fitted with Pantsir-M (Odintsovo and later); the first
    two units had AK-630M. **Gepard** has a helicopter pad but no hangar;
    a deck of capacity 1 is used so the pad helo is representable, which
    overstates sustained aviation support.
19. **Novorossiysk** has no Kinzhal (the volume says the system was never
    fitted); the fit was removed rather than copied from sister ships.
20. **SLBM tubes** on Borei, Borei II, Delta IV and Typhoon are omitted
    because the engine has no SLBM weapon kind. The tube count and missile
    type are recorded here and in the platform name (`[no SLBM]`); no
    fictional cruise/ballistic substitute was added.
21. **Carrier air-wing class ids** use `ru_cv_` (`ru_cv_su33`,
    `ru_cv_mig29kr`, `ru_cv_ka27`, `ru_cv_ka29`, `ru_cv_ka31r`,
    `ru_cv_ka52k`). The plain `ru_` + type ids already belong to the
    dedicated `ru_aircraft.json` volume; the `ru_cv_` ids keep this volume
    self-contained for `dbcheck --only` without overriding that volume's
    richer aircraft entries during the startup merge (files merge in
    filename order, `ru_navy` after `ru_aircraft`).

## Verification

- `cargo run --release -p harpoon-server --example dbcheck -- --only database/volumes/ru_navy.json`
  → `DBCHECK PASS — merged: 91 platforms, 53 weapons, 20 sensors`
  (volume contribution: +55 platforms, +30 weapons, +6 sensors).
- `cargo run --release -p harpoon-server --example dbcheck` (all volumes)
  → `DBCHECK PASS — merged: 377 platforms, 222 weapons, 147 sensors`.
- `cargo test -p harpoon-core` → `89 passed; 0 failed`.
