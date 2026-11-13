// Проход наложения одной записи (ADR-0014): ячейки клиентского буфера поверх текущего содержимого
// текстуры наложения — бит-идентично `draw_overlay_cell` в soft_backend.rs. Текстура размером с
// сетку в пикселях, тексель (x, y) — пиксель сетки (x, y) от левого верхнего угла; RGB предумножен.
// Заголовок #version (и precision для ES) подставляет программа.

uniform highp usampler2D u_prev;   // RGBA8UI: результат предыдущих записей (r, g, b предумножены, a)
uniform highp usampler2D u_layer;  // RGBA32UI: ячейки записи, как u_data основного прохода (§6.1)
uniform highp usampler2D u_atlas;  // RGBA8UI: каналы — начертания
uniform ivec2 u_layer_origin;      // ячейка основной сетки под левым верхним углом записи
uniform ivec2 u_layer_size;        // cols, rows записи
uniform ivec2 u_glyph;             // размер ячейки в пикселях
uniform int u_atlas_cols;          // слотов в строке атласа
uniform ivec4 u_style_channel;     // канал атласа для начертаний

out uvec4 o_texel;

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

// Слой (rgb, a) поверх t — `over` в soft_backend.rs.
uvec4 over(uvec4 t, uvec3 c, uint a) {
    uint inv = 255u - a;
    return uvec4((c * a + 127u) / 255u + (t.rgb * inv + 127u) / 255u, a + (t.a * inv + 127u) / 255u);
}

void main() {
    ivec2 p = ivec2(gl_FragCoord.xy);
    uvec4 t = texelFetch(u_prev, p, 0);
    ivec2 grid_cell = p / u_glyph;
    ivec2 cell = grid_cell - u_layer_origin;
    if (cell.x < 0 || cell.y < 0 || cell.x >= u_layer_size.x || cell.y >= u_layer_size.y) {
        o_texel = t;
        return;
    }
    ivec2 in_cell = p - grid_cell * u_glyph;

    uvec4 d = texelFetch(u_layer, cell, 0);
    uint slot = d.r & 0xFFFFu;
    uint fl = d.r >> 16u;
    uvec3 fg = rgb(d.g);
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

    t = over(t, rgb(d.b), alpha(d.b));
    o_texel = over(t, fg, (a * alpha(d.g) + 127u) / 255u);
}
