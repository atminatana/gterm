# Темы примеров `gterm-widgets`

Файлы формата тем редактора helix: область → цвет символа или таблица `{ fg, bg, modifiers }`,
имена цветов — в `[palette]` (ADR-0037). Тема читается на трёх уровнях, у каждого элемента есть
имя, и любой из них можно задать строкой в файле (ADR-0038):

1. **основа** — `ui.background`, `ui.text`, `ui.accent` (акцент и текст на нём), `error`;
2. **тона** — `ui.control`, `ui.control.focus`, `ui.control.pressed`, `ui.control.disabled`,
   `ui.selection`, `ui.popup`, `ui.window`, `ui.window.focus`, `ui.text.focus`,
   `ui.text.inactive`;
3. **области виджетов** — `ui.button*`, `ui.input*`, `ui.checkbox*`, `ui.combo*`,
   `ui.list.selected`, `ui.table*`, `ui.tabs*`, `ui.scrollbar*`, `ui.gauge*`.

Чего в файле нет, выводится из уровня выше. Посмотреть результат — пример `theme`
(`cargo run -p gterm-widgets --example theme`): выведенные области помечены знаком `≈`; `F6` сохраняет тему на экране полной записью — файлом, где заданы
все области, включая выведенные: его можно править или взять за основу своей темы.

## Свои темы

`gterm_dark.toml`, `gterm_light.toml` — образцы точечной настройки: области виджетов заданы
явно. Лицензия проекта — `MIT OR Apache-2.0`.

## Темы sniffnet

`sniffnet_*.toml` — палитры проекта [Sniffnet](https://github.com/GyulyVGC/sniffnet) версии
1.6.0 (коммит `247b75b`), лицензия Sniffnet — `MIT OR Apache-2.0`. Перенесены только цвета: шесть
цветов палитры каждой темы записаны в `[palette]` под именами Sniffnet (`primary`, `secondary`,
`outgoing`, `text_body`, `text_headers`, `starred`), из них заданы три области основы — остальное
выводится.

| Файлы | Источник в репозитории Sniffnet |
|---|---|
| `sniffnet_catppuccin`, `sniffnet_deep_cosmos`, `sniffnet_deep_sea`, `sniffnet_electric_wave`, `sniffnet_goleador`, `sniffnet_heat_waves`, `sniffnet_light_bubbles`, `sniffnet_mon_amour`, `sniffnet_seasons`, `sniffnet_toucan` | `resources/themes/*.toml` |
| `sniffnet_a11y_*`, `sniffnet_dracula_*`, `sniffnet_gruvbox_*`, `sniffnet_nord_*`, `sniffnet_solarized_*`, `sniffnet_yeti_*` (`dark` и `light`) | `src/gui/styles/custom_themes/*.rs` |

`outgoing` и `starred` — цвета самого Sniffnet; библиотека их не использует, приложение читает
их через `Theme::color`.
