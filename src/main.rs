use mantis::{draw_crosshair, draw_line_3d, Bounds, Camera, CharacterController, Engine, Game, Input, Vec3};

const LIME_GREEN: u32 = 0x00FF00;
const MOUSE_SENSITIVITY: f32 = 0.003;
const MOVE_SPEED: f32 = 0.15;
const SPRINT_MULTIPLIER: f32 = 2.0;
const JUMP_FORCE: f32 = 0.3;
const GRAVITY: f32 = 0.015;

// Room dimensions
const ROOM_W: f32 = 40.0;
const ROOM_H: f32 = 10.0;
const ROOM_D: f32 = 40.0;

struct MyGame {
    camera: Camera,
    controller: CharacterController,
    vertices: [Vec3; 8],
    edges: [(usize, usize); 12],
    bounds: Bounds,
}

impl MyGame {
    fn new(aspect: f32) -> Self {
        let hw = ROOM_W / 2.0;
        let hh = ROOM_H / 2.0;
        let hd = ROOM_D / 2.0;

        // 8 corners of the room (rectangular prism centered at origin)
        let vertices = [
            Vec3::new(-hw, -hh, -hd), // 0: left  bottom front
            Vec3::new( hw, -hh, -hd), // 1: right bottom front
            Vec3::new( hw,  hh, -hd), // 2: right top    front
            Vec3::new(-hw,  hh, -hd), // 3: left  top    front
            Vec3::new(-hw, -hh,  hd), // 4: left  bottom back
            Vec3::new( hw, -hh,  hd), // 5: right bottom back
            Vec3::new( hw,  hh,  hd), // 6: right top    back
            Vec3::new(-hw,  hh,  hd), // 7: left  top    back
        ];

        // 12 edges of the rectangular prism
        let edges = [
            // Front face
            (0, 1), (1, 2), (2, 3), (3, 0),
            // Back face
            (4, 5), (5, 6), (6, 7), (7, 4),
            // Connecting edges
            (0, 4), (1, 5), (2, 6), (3, 7),
        ];

        let camera = Camera::new(Vec3::new(0.0, 0.0, 0.0), aspect);
        let controller = CharacterController::new(MOVE_SPEED, SPRINT_MULTIPLIER, JUMP_FORCE, GRAVITY, 0.0);

        // Keep player inside the room with a small margin from the walls
        let margin = 0.5;
        let bounds = Bounds::new(
            Vec3::new(-hw + margin, -hh + margin, -hd + margin),
            Vec3::new( hw - margin,  hh - margin,  hd - margin),
        );

        MyGame {
            camera,
            controller,
            vertices,
            edges,
            bounds,
        }
    }
}

impl Game for MyGame {
    fn update(&mut self, input: &Input) {
        self.camera.rotate(input.mouse_dx, input.mouse_dy, MOUSE_SENSITIVITY);
        self.controller.update(&mut self.camera, input);
        self.bounds.clamp(&mut self.camera.position);
    }

    fn render(&mut self, buffer: &mut Vec<u32>, width: usize, height: usize) {
        for &(a, b) in &self.edges {
            draw_line_3d(
                buffer,
                width,
                height,
                &self.camera,
                self.vertices[a],
                self.vertices[b],
                LIME_GREEN,
            );
        }

        draw_crosshair(buffer, width, height, 10, 0xFFFFFF);
    }
}

fn main() {
    let mut engine = Engine::new("Example Mantis Game");
    let aspect = engine.width() as f32 / engine.height() as f32;
    let mut game = MyGame::new(aspect);
    engine.run(&mut game);
}
