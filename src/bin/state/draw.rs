use crate::render::Geometry;
use crate::render::WgpuState;

pub struct DrawState {
    changed: bool,

    height: u32,
    stroke_width: f32,
    stroke_color: csscolorparser::Color,
    color_needs_pre_multiply: bool,
    current_line: Vec<(f32, f32)>,
    tessellated_lines: Vec<Geometry>,
    tessellated_lines_source: Vec<lyon::path::Path>,
    /// Stroked ring at the active drawing finger; drawn above ink.
    touch_cursor: Option<Geometry>,
}

impl DrawState {
    pub fn new(stroke_width: f32, stroke_color: csscolorparser::Color) -> Self {
        Self {
            changed: false,
            height: 0,
            stroke_width,
            stroke_color,
            color_needs_pre_multiply: false,
            current_line: Vec::new(),
            tessellated_lines: Vec::new(),
            tessellated_lines_source: Vec::new(),
            touch_cursor: None,
        }
    }

    pub fn set_height(&mut self, height: u32) {
        self.height = height;
    }

    pub fn set_stroke_width(&mut self, width: f32) {
        self.stroke_width = width;
    }

    pub fn set_stroke_color(&mut self, color: csscolorparser::Color) {
        self.stroke_color = color;
        if self.color_needs_pre_multiply {
            self.pre_multiply_stroke_color();
        }
    }

    pub fn set_pre_multiply_stroke_color(&mut self, b: bool) {
        if b {
            self.pre_multiply_stroke_color();
        }
        self.color_needs_pre_multiply = b;
    }

    fn pre_multiply_stroke_color(&mut self) {
        self.stroke_color.r *= self.stroke_color.a;
        self.stroke_color.g *= self.stroke_color.a;
        self.stroke_color.b *= self.stroke_color.a;
    }

    pub fn render(&mut self, wgpu: &WgpuState) {
        if self.changed {
            self.force_render(wgpu);
        }
    }

    pub fn force_render(&mut self, wgpu: &WgpuState) {
        wgpu.render(
            self.tessellated_lines
                .iter()
                .chain(self.tessellate_current_line().map(|(g, _)| g).iter())
                .chain(self.touch_cursor.iter()),
        );

        self.changed = false;
    }

    pub fn add_point_to_line(&mut self, pos: (f64, f64)) {
        self.add_point_to_line_with_epsilon(pos, crate::EPSILON);
    }

    pub fn add_point_to_line_touch(&mut self, pos: (f64, f64)) {
        self.add_point_to_line_with_epsilon(pos, crate::TOUCH_EPSILON);
    }

    pub fn add_point_to_line_with_epsilon(&mut self, (mouse_x, mouse_y): (f64, f64), epsilon: f32) {
        let new_x = mouse_x as f32;
        let new_y = self.height as f32 - mouse_y as f32;
        match self.current_line.last() {
            Some((x, y)) => {
                if f32::abs(x - new_x) + f32::abs(y - new_y) > epsilon {
                    self.current_line.push((new_x, new_y));
                    self.changed = true;
                }
            }
            None => {
                self.current_line.push((new_x, new_y));
                self.changed = true;
            }
        }

        // lines shouldn't get *too* long or it'll cause performance issues
        // also lyon has an upper limit at some point
        if self.current_line.len() > 0x800 {
            let (line, path) = self.tessellate_current_line().unwrap();
            self.tessellated_lines.push(line);
            self.tessellated_lines_source.push(path);
            self.current_line.clear();
            self.changed = true;
        }
    }

    pub fn cut_line(&mut self) {
        if let Some((tesselated_line, path)) = self.tessellate_current_line() {
            self.tessellated_lines.push(tesselated_line);
            self.tessellated_lines_source.push(path);
        }
        self.current_line.clear();
    }

    pub fn undo(&mut self) {
        if self.current_line.is_empty() {
            self.tessellated_lines.pop();
            self.tessellated_lines_source.pop();
        } else {
            self.current_line.clear();
        }

        self.changed = true;
    }

    pub fn clear(&mut self) {
        self.tessellated_lines.clear();
        self.tessellated_lines_source.clear();
        self.current_line.clear();

        self.changed = true;
    }

    pub fn set_touch_drawing_cursor_surface(&mut self, surface: Option<(f64, f64)>) {
        let next = surface.map(|(sx, sy)| self.tessellate_touch_cursor_ring(sx, sy));
        if self.touch_cursor.is_none() && next.is_none() {
            return;
        }
        self.touch_cursor = next;
        self.changed = true;
    }

    fn cursor_indicator_color(&self) -> csscolorparser::Color {
        let mut c = csscolorparser::parse("#D0FFFFFF").unwrap();
        if self.color_needs_pre_multiply {
            c.r *= c.a;
            c.g *= c.a;
            c.b *= c.a;
        }
        c
    }

    fn tessellate_touch_cursor_ring(&self, sx: f64, sy: f64) -> Geometry {
        use crate::render::Vertex;
        use lyon::math::point;
        use lyon::path::Path;
        use lyon::tessellation::BuffersBuilder;
        use lyon::tessellation::StrokeOptions;
        use lyon::tessellation::StrokeTessellator;
        use lyon::tessellation::StrokeVertex;
        use lyon::tessellation::VertexBuffers;

        let cx = sx as f32;
        let cy = self.height as f32 - sy as f32;
        let radius = (self.stroke_width * 2.0).clamp(10.0, 28.0);
        let ring_width = (self.stroke_width * 0.35).clamp(1.5, 3.5);

        let segments = 40u32;
        let mut builder = Path::builder();
        for i in 0..=segments {
            let t = i as f32 / segments as f32 * std::f32::consts::TAU;
            let x = cx + radius * t.cos();
            let y = cy + radius * t.sin();
            if i == 0 {
                builder.begin(point(x, y));
            } else {
                builder.line_to(point(x, y));
            }
        }
        builder.end(true);
        let path = builder.build();

        let color = self.cursor_indicator_color();
        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();
        let mut tessellator = StrokeTessellator::new();
        let stroke_options = StrokeOptions::default()
            .with_line_width(ring_width)
            .with_line_cap(lyon::path::LineCap::Round)
            .with_line_join(lyon::path::LineJoin::Round);

        tessellator
            .tessellate_path(
                &path,
                &stroke_options,
                &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
                    Vertex::new(vertex, &color)
                }),
            )
            .unwrap();

        Geometry::new(geometry)
    }

    pub fn erase(&mut self, (mouse_x, mouse_y): (f64, f64)) {
        let x = mouse_x as f32;
        let y = self.height as f32 - mouse_y as f32;

        let p = lyon::math::point(x, y);

        let eraser_size = self.stroke_width * 10.0;

        let mut to_remove = None;

        for (i, line) in self.tessellated_lines_source.iter().enumerate() {
            // simple distance check from each point to our cursor
            // we could also use lyon::math::hit_test
            // but that has caused problems with short paths
            for event in line {
                match event {
                    lyon::path::Event::Begin { at } => {
                        if (at - p).square_length() < eraser_size {
                            to_remove = Some(i);
                            break;
                        }
                    }
                    lyon::path::Event::Line { from: _, to } => {
                        if (to - p).square_length() < eraser_size {
                            to_remove = Some(i);
                            break;
                        }
                    }
                    lyon::path::Event::End {
                        last: _,
                        first: _,
                        close: _,
                    } => {}
                    lyon::path::Event::Quadratic {
                        from: _,
                        ctrl: _,
                        to: _,
                    } => unreachable!(),
                    lyon::path::Event::Cubic {
                        from: _,
                        ctrl1: _,
                        ctrl2: _,
                        to: _,
                    } => unreachable!(),
                }
            }
        }

        if let Some(i) = to_remove {
            self.tessellated_lines.remove(i);
            self.tessellated_lines_source.remove(i);

            self.changed = true;
        }
    }

    fn tessellate_current_line(&self) -> Option<(Geometry, lyon::path::Path)> {
        use crate::render::Vertex;
        use lyon::math::point;
        use lyon::path::Path;
        use lyon::tessellation::BuffersBuilder;
        use lyon::tessellation::StrokeOptions;
        use lyon::tessellation::StrokeTessellator;
        use lyon::tessellation::StrokeVertex;
        use lyon::tessellation::VertexBuffers;

        let line = &self.current_line;

        let mut geometry: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        let mut builder = Path::builder();
        if line.len() < 1 {
            return None;
        }

        builder.begin(point(line[0].0, line[0].1));
        // small hack for drawing dots
        builder.line_to(point(line[0].0, line[0].1));
        for &(x, y) in line.iter().skip(1) {
            builder.line_to(point(x, y));
        }
        builder.end(false);
        let path = builder.build();

        let mut tessellator = StrokeTessellator::new();
        let stroke_options = StrokeOptions::default()
            .with_line_width(self.stroke_width)
            .with_line_cap(lyon::path::LineCap::Round)
            .with_line_join(lyon::path::LineJoin::Round);

        tessellator
            .tessellate_path(
                &path,
                &stroke_options,
                &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
                    Vertex::new(vertex, &self.stroke_color)
                }),
            )
            .unwrap();

        Some((Geometry::new(geometry), path))
    }
}
