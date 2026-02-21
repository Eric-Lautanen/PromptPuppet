<div align="center">

# 🎭 PromptPuppet

**A visual pose editor and semantic prompt generator for AI image & video creation**

[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Windows-blue)](https://github.com/Eric-Lautanen/PromptPuppet/releases)

Transform 3D stick-figure poses into rich, anatomically accurate text descriptions for **Stable Diffusion**, **Midjourney**, **Sora**, **Grok-Imagine** and other AI image/video generators.

<img src="assets/screenshot.png" alt="PromptPuppet" width="80%">

**Download** the latest release from [Releases](https://github.com/Eric-Lautanen/PromptPuppet/releases)

</div>

---

## ✨ Features

### 🎨 **Visual 3D Pose Editor**
- **Interactive 3D Canvas** – Click and drag to position joints in real-time with perspective projection
- **Full FABRIK Inverse Kinematics** – Anatomically correct bone-length-preserving IK for all limbs, with ragdoll-style gravity sag when moving the neck/torso
- **Camera Control** – Orbit and zoom the 3D viewport freely around your pose
- **Anatomical Precision** – Full-body rigging with shoulders, elbows, wrists, hands, hips, knees, ankles, and feet
- **Advanced Controls** – Fine-tune head tilt/nod/yaw, torso lean/sway, and individual finger positions
- **Preset Library** – Load professional poses, expressions, and character stances instantly

### 📝 **Smart Prompt Generation**
- **Semantic Translation** – Automatically converts visual joint positions into detailed natural language descriptions the moment you manually adjust a pose
- **Preset-to-Semantic Handoff** – Uses JSON preset prompts when a known pose is loaded; switches to live semantic description as soon as you drag a joint
- **Multi-Modal Support** – Separate image and video mode configurations
- **Comprehensive Attributes** – Character traits, clothing, styles, environments, motion, and settings
- **Conditional Logic** – Dynamic UI elements that adapt based on your selections
- **Stable Output Order** – Prompt text is always generated in a consistent, deterministic order regardless of internal map iteration
- **Live Updates** – Prompt rewrites as you drag — no button needed
- **One-Click Export** – Copy complete prompts directly to your clipboard

### 🎬 **Video & Image Modes**
- **Image Mode** – Optimized for single-frame generation (Midjourney, DALL-E, Stable Diffusion)
- **Video Mode** – Motion parameters for video generators (Sora, Runway, Pika)
- **Mode-Aware Prompts** – Automatically includes/excludes relevant parameters per mode

### 💾 **Project Management**
- **Save/Load States** – Timestamped snapshots with atomic file writes (crash-safe — a failed save never corrupts existing saves)
- **Custom Presets** – Create and save your own character configurations
- **Search & Filter** – Quickly find poses, expressions, and styles from extensive libraries

### 🎯 **Professional Workflow**
- **JSON-Driven UI** – Fully customizable interface via configuration files
- **Zero External Files** – Single self-contained executable with all assets embedded at compile time
- **Native Performance** – Built in Rust using egui; prompt rebuilds only when state actually changes
- **Windows Native** – Optimized executable for Windows 10/11

---

## 🚀 Quick Start

### Installation

1. **Download** the latest release from [Releases](https://github.com/Eric-Lautanen/PromptPuppet/releases)
2. **Run** the executable – No installation required
3. **Start Creating** – The app is fully self-contained with all assets embedded

### Basic Workflow

1. **Pose Your Character**
   - Drag joints on the 3D canvas to create your desired pose
   - Use the preset library for common poses (standing, sitting, action poses)
   - Orbit the camera to check your pose from any angle

2. **Configure Attributes**
   - Select character features (gender, age, build, ethnicity)
   - Choose clothing, hairstyle, and accessories
   - Pick artistic style and environment settings

3. **Generate & Copy**
   - Review the auto-generated prompt in the bottom panel
   - Click "Copy to Clipboard" to use in your AI generator
   - Save your state to reuse later

---

## 🎯 Use Cases

### For AI Artists
- **Character Consistency** – Maintain consistent poses across multiple generations
- **Rapid Iteration** – Quickly test different poses without rewriting prompts
- **Style Exploration** – Combine poses with different artistic styles and environments

### For Game Developers
- **Concept Art** – Generate reference images for character designs and animations
- **Storyboarding** – Create quick visual mockups with precise character positions

### For Content Creators
- **Social Media** – Generate consistent character poses for branded content
- **Animation Pre-viz** – Plan video sequences with frame-by-frame pose references

### For Educators & Students
- **Anatomy Practice** – Study and reference proper body proportions and poses
- **Portfolio Work** – Create professional character sheets and pose studies

---

## 🛠️ Technical Stack

- **Language:** Rust
- **GUI Framework:** [egui](https://github.com/emilk/egui) (immediate mode UI)
- **Rendering:** [eframe](https://github.com/emilk/egui/tree/master/crates/eframe)
- **Serialization:** serde + serde_json
- **Image Processing:** [image](https://github.com/image-rs/image)

### Architecture Highlights

- **Compile-Time Asset Embedding** – All JSON configs, fonts, and icons bundled via `include_str!` and `include_bytes!`; single-executable deployment
- **FABRIK IK System** – Forward And Backward Reaching IK maintains bone lengths across all limb chains; ragdoll solver for whole-body neck drags with weighted gravity sag per joint
- **3D Viewport** – Perspective projection with mouse-driven orbit camera and ray-cast joint picking
- **Semantic Pose Engine** – Joint positions are continuously interpreted and described in natural language, seamlessly replacing preset text when the user takes manual control
- **Reactive State Management** – Prompt regenerates only when `AppState` actually changes (hash-based dirty check), keeping the UI fast even during continuous interaction
- **Crash-Safe Saves** – All state writes go to a sibling `.tmp` file first, then atomically renamed into place
- **Modular JSON Configuration** – UI panels, presets, options, and settings are all data-driven and easy to extend

---

## 📚 Configuration

PromptPuppet uses JSON files (embedded at compile time) to define:

- **UI Layout** (`ui_config.json`) – Panel structure and component hierarchy
- **Character Attributes** (`character_attributes.json`) – Physical features and demographics
- **Clothing & Accessories** (`clothing.json`) – Wardrobe options
- **Poses & Expressions** (`poses.json`, `expressions.json`) – Preset pose library
- **Styles** (`styles.json`) – Artistic styles with positive/negative prompts
- **Environments** (`environments.json`) – Background settings
- **Motion** (`motion.json`) – Video-specific motion parameters
- **Global Settings** (`global.json`) – Camera angles, lighting, composition
- **Skeleton** (`skeleton.json`) – Bone lengths, joint definitions, and angle constraints

---

## 🎨 Supported AI Platforms

PromptPuppet generates prompts compatible with:

- **Stable Diffusion** (SD 1.5, SDXL, SD3)
- **Midjourney** (v5, v6, niji)
- **DALL-E** (2, 3)
- **Sora** (OpenAI's video model)
- **Runway** (Gen-2, Gen-3)
- **Pika Labs**
- **Leonardo AI**
- **Playground AI**
- **Any text-to-image/video model accepting natural language prompts**

---

## 🗺️ Roadmap

The core feature set is complete. Ongoing work is focused on UI polish and performance:

- [ ] **Linux & macOS Support** – Cross-platform builds
- [ ] **UI Polish** – Layout refinements, improved joint highlighting, and better visual feedback
- [ ] **Performance** – Rendering optimizations for high-DPI displays and large preset libraries
- [ ] **Pose Import** – Load from OpenPose JSON, MediaPipe, or other standard formats
- [ ] **Animation Timeline** – Keyframe-based pose sequences for video workflows

---

## 🤝 Contributing

Contributions welcome! Whether it's bug reports, feature requests, or pull requests:

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

---

## 📄 License

MIT License

Copyright (c) 2026 Eric Lautanen

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

---

## 🙏 Acknowledgments

- Built with [egui](https://github.com/emilk/egui) by Emil Ernerfeldt
- Inspired by pose estimation tools like OpenPose and MediaPipe
- Icon and UI design by the PromptPuppet team
- & Claude!!!

---

## 📬 Contact & Support

- **GitHub Issues:** [Report bugs or request features](https://github.com/Eric-Lautanen/PromptPuppet/issues)
- **Discussions:** [Join the community](https://github.com/Eric-Lautanen/PromptPuppet/discussions)

---

## 🌟 Star History

If you find PromptPuppet useful, please consider giving it a star ⭐ on GitHub!

---

**Made with ❤️ for the AI art community**