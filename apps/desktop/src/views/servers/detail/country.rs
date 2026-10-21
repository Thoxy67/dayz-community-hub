//! A country's name in the interface's language, from its ISO 3166-1 alpha-2 code
//! (`Intl.DisplayNames` in the Svelte panel). The countries DayZ servers are hosted in;
//! any other reads as its code.

use crate::i18n::{self, Locale};

/// (code, en, fr, de, es, ru).
const NAMES: &[(&str, [&str; 5])] = &[
    (
        "US",
        [
            "United States",
            "États-Unis",
            "Vereinigte Staaten",
            "Estados Unidos",
            "США",
        ],
    ),
    ("CA", ["Canada", "Canada", "Kanada", "Canadá", "Канада"]),
    (
        "BR",
        ["Brazil", "Brésil", "Brasilien", "Brasil", "Бразилия"],
    ),
    (
        "GB",
        [
            "United Kingdom",
            "Royaume-Uni",
            "Vereinigtes Königreich",
            "Reino Unido",
            "Великобритания",
        ],
    ),
    (
        "IE",
        ["Ireland", "Irlande", "Irland", "Irlanda", "Ирландия"],
    ),
    (
        "FR",
        ["France", "France", "Frankreich", "Francia", "Франция"],
    ),
    (
        "DE",
        [
            "Germany",
            "Allemagne",
            "Deutschland",
            "Alemania",
            "Германия",
        ],
    ),
    (
        "NL",
        [
            "Netherlands",
            "Pays-Bas",
            "Niederlande",
            "Países Bajos",
            "Нидерланды",
        ],
    ),
    (
        "BE",
        ["Belgium", "Belgique", "Belgien", "Bélgica", "Бельгия"],
    ),
    (
        "LU",
        [
            "Luxembourg",
            "Luxembourg",
            "Luxemburg",
            "Luxemburgo",
            "Люксембург",
        ],
    ),
    (
        "CH",
        ["Switzerland", "Suisse", "Schweiz", "Suiza", "Швейцария"],
    ),
    (
        "AT",
        ["Austria", "Autriche", "Österreich", "Austria", "Австрия"],
    ),
    ("IT", ["Italy", "Italie", "Italien", "Italia", "Италия"]),
    ("ES", ["Spain", "Espagne", "Spanien", "España", "Испания"]),
    (
        "PT",
        ["Portugal", "Portugal", "Portugal", "Portugal", "Португалия"],
    ),
    ("PL", ["Poland", "Pologne", "Polen", "Polonia", "Польша"]),
    (
        "CZ",
        ["Czechia", "Tchéquie", "Tschechien", "Chequia", "Чехия"],
    ),
    (
        "SK",
        [
            "Slovakia",
            "Slovaquie",
            "Slowakei",
            "Eslovaquia",
            "Словакия",
        ],
    ),
    ("HU", ["Hungary", "Hongrie", "Ungarn", "Hungría", "Венгрия"]),
    (
        "RO",
        ["Romania", "Roumanie", "Rumänien", "Rumanía", "Румыния"],
    ),
    (
        "BG",
        ["Bulgaria", "Bulgarie", "Bulgarien", "Bulgaria", "Болгария"],
    ),
    (
        "DK",
        ["Denmark", "Danemark", "Dänemark", "Dinamarca", "Дания"],
    ),
    ("SE", ["Sweden", "Suède", "Schweden", "Suecia", "Швеция"]),
    (
        "NO",
        ["Norway", "Norvège", "Norwegen", "Noruega", "Норвегия"],
    ),
    (
        "FI",
        ["Finland", "Finlande", "Finnland", "Finlandia", "Финляндия"],
    ),
    (
        "EE",
        ["Estonia", "Estonie", "Estland", "Estonia", "Эстония"],
    ),
    (
        "LV",
        ["Latvia", "Lettonie", "Lettland", "Letonia", "Латвия"],
    ),
    (
        "LT",
        ["Lithuania", "Lituanie", "Litauen", "Lituania", "Литва"],
    ),
    (
        "UA",
        ["Ukraine", "Ukraine", "Ukraine", "Ucrania", "Украина"],
    ),
    (
        "BY",
        [
            "Belarus",
            "Biélorussie",
            "Belarus",
            "Bielorrusia",
            "Беларусь",
        ],
    ),
    ("RU", ["Russia", "Russie", "Russland", "Rusia", "Россия"]),
    (
        "KZ",
        [
            "Kazakhstan",
            "Kazakhstan",
            "Kasachstan",
            "Kazajistán",
            "Казахстан",
        ],
    ),
    ("TR", ["Türkiye", "Turquie", "Türkei", "Turquía", "Турция"]),
    (
        "GR",
        ["Greece", "Grèce", "Griechenland", "Grecia", "Греция"],
    ),
    ("RS", ["Serbia", "Serbie", "Serbien", "Serbia", "Сербия"]),
    (
        "HR",
        ["Croatia", "Croatie", "Kroatien", "Croacia", "Хорватия"],
    ),
    (
        "SI",
        ["Slovenia", "Slovénie", "Slowenien", "Eslovenia", "Словения"],
    ),
    (
        "AU",
        [
            "Australia",
            "Australie",
            "Australien",
            "Australia",
            "Австралия",
        ],
    ),
    (
        "NZ",
        [
            "New Zealand",
            "Nouvelle-Zélande",
            "Neuseeland",
            "Nueva Zelanda",
            "Новая Зеландия",
        ],
    ),
    ("JP", ["Japan", "Japon", "Japan", "Japón", "Япония"]),
    (
        "KR",
        [
            "South Korea",
            "Corée du Sud",
            "Südkorea",
            "Corea del Sur",
            "Республика Корея",
        ],
    ),
    (
        "SG",
        ["Singapore", "Singapour", "Singapur", "Singapur", "Сингапур"],
    ),
    (
        "HK",
        ["Hong Kong", "Hong Kong", "Hongkong", "Hong Kong", "Гонконг"],
    ),
    ("IN", ["India", "Inde", "Indien", "India", "Индия"]),
    (
        "ZA",
        [
            "South Africa",
            "Afrique du Sud",
            "Südafrika",
            "Sudáfrica",
            "ЮАР",
        ],
    ),
    ("MX", ["Mexico", "Mexique", "Mexiko", "México", "Мексика"]),
    (
        "AR",
        [
            "Argentina",
            "Argentine",
            "Argentinien",
            "Argentina",
            "Аргентина",
        ],
    ),
    ("CL", ["Chile", "Chili", "Chile", "Chile", "Чили"]),
];

/// The name of `code` in the current language; the code itself when not known.
pub fn name(code: &str) -> String {
    let up = code.trim().to_ascii_uppercase();
    let i = match i18n::locale() {
        Locale::En => 0,
        Locale::Fr => 1,
        Locale::De => 2,
        Locale::Es => 3,
        Locale::Ru => 4,
    };
    NAMES
        .iter()
        .find(|(c, _)| *c == up)
        .and_then(|(_, n)| n.get(i).copied())
        .map_or(up, str::to_string)
}
