// Полноэкранный треугольник по gl_VertexID (ТЗ §7.1): без vertex buffer, VAO пустой.
// Заголовок #version подставляет программа.
void main() {
    vec2 v = vec2(gl_VertexID == 1 ? 3.0 : -1.0, gl_VertexID == 2 ? 3.0 : -1.0);
    gl_Position = vec4(v, 0.0, 1.0);
}
