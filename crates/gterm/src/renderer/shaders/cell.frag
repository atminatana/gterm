// Фрагментный шейдер ячеек — алгоритм ТЗ §7.3, бит-идентичный soft_backend.rs (ADR-0002/0003/0005/0013).
// Заголовок #version (и precision для ES) подставляет программа.

uniform highp usampler2D u_data;   // RGBA32UI cols×rows: R = слот | флаги << 16, G = fg, B = bg — 0xAARRGGBB (§6.1)
uniform highp usampler2D u_atlas;  // RGBA8UI: каналы — начертания, покрытие 0…255 (§6.3)
uniform ivec2 u_grid;              // cols, rows
uniform ivec2 u_glyph;             // размер ячейки в пикселях (FR-10, ADR-0005)
uniform ivec2 u_origin;            // смещение сетки в окне (центрирование)
uniform ivec3 u_background;        // фон буфера RGB 0…255: подложка ячеек и поле вокруг сетки (ADR-0013)
uniform int u_viewport_h;          // высота окна в пикселях
uniform int u_atlas_cols;          // слотов в строке атласа
uniform ivec4 u_style_channel;     // канал атласа для начертаний Regular/Bold/Italic/BoldItalic (с подстановкой FR-8.3)
uniform highp usampler2D u_overlay; // RGBA8UI размером с сетку: наложение, RGB предумножен (ADR-0014)
uniform int u_has_overlay;          // 1 — в этом кадре есть наложение

out vec4 o_color;

const uint F_BOLD = 1u;
const uint F_ITALIC = 2u;
const uint F_UNDERLINE = 4u;
const uint BOLD_BOOST = 0x55u;

uvec3 rgb(uint c) {
    return uvec3((c >> 16u) & 255u, (c >> 8u) & 255u, c & 255u);
}

uint alpha(uint c) {
    return c >> 24u;
}

// Целочисленное смешивание по каналам (ADR-0002) — `blend` в soft_backend.rs.
uvec3 blend(uvec3 under, uvec3 over, uint a) {
    return (under * (255u - a) + over * a + 127u) / 255u;
}

vec4 to_color(uvec3 c) {
    return vec4(vec3(c) / 255.0, 1.0);
}

void main() {
    uvec3 back = uvec3(u_background);
    ivec2 p = ivec2(int(gl_FragCoord.x), u_viewport_h - 1 - int(gl_FragCoord.y)) - u_origin;
    if (p.x < 0 || p.y < 0) {
        o_color = to_color(back);
        return;
    }
    ivec2 cell = p / u_glyph;
    if (cell.x >= u_grid.x || cell.y >= u_grid.y) {
        o_color = to_color(back);
        return;
    }
    ivec2 in_cell = p - cell * u_glyph;

    uvec4 d = texelFetch(u_data, cell, 0);
    uint slot = d.r & 0xFFFFu;
    uint fl = d.r >> 16u;
    uvec3 fg = rgb(d.g);
    // Фон ячейки поверх фона буфера по своей альфе.
    uvec3 bg = blend(back, rgb(d.b), alpha(d.b));

    if ((fl & F_BOLD) != 0u) {
        fg = min(fg + uvec3(BOLD_BOOST), uvec3(255u));
    }

    int style = ((fl & F_BOLD) != 0u ? 1 : 0) + ((fl & F_ITALIC) != 0u ? 2 : 0);
    int channel = u_style_channel[style];
    ivec2 g = ivec2(int(slot) % u_atlas_cols, int(slot) / u_atlas_cols) * u_glyph + in_cell;
    uint a = texelFetch(u_atlas, g, 0)[channel];

    if ((fl & F_UNDERLINE) != 0u && in_cell.y == u_glyph.y - 1) {
        a = 255u;
    }
    // Покрытие × альфа символа — `fade` в soft_backend.rs.
    a = (a * alpha(d.g) + 127u) / 255u;

    uvec3 c = blend(bg, fg, a);
    if (u_has_overlay != 0) {
        // Текстура наложения поверх кадра — `composite` в soft_backend.rs.
        uvec4 t = texelFetch(u_overlay, p, 0);
        c = t.rgb + (c * (255u - t.a) + 127u) / 255u;
    }
    o_color = to_color(c);
}
