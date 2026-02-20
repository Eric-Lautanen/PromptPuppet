# 🎭 PromptPuppet

**A visual pose editor and semantic prompt generator for AI image & video creation**

[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Windows-blue)](https://github.com/Eric-Lautanen/PromptPuppet/releases)

Transform 2D stick-figure poses into rich, anatomically accurate text descriptions for **Stable Diffusion**, **Midjourney**, **Sora**, and other AI image/video generators.

---

## ✨ Features

### 🎨 **Visual Pose Editor**
- **Interactive Canvas** – Click and drag to position joints in real-time
- **Anatomical Precision** – Full-body rigging with shoulders, elbows, wrists, hands, hips, knees, ankles, and feet
- **Advanced Controls** – Fine-tune head tilt/nod/yaw, torso lean/sway, and individual finger positions
- **Preset Library** – Load professional poses, expressions, and character stances instantly

### 📝 **Smart Prompt Generation**
- **Semantic Translation** – Converts visual poses into detailed natural language descriptions
- **Multi-Modal Support** – Separate image and video mode configurations
- **Comprehensive Attributes** – Character traits, clothing, styles, environments, motion, and settings
- **Conditional Logic** – Dynamic UI elements that adapt based on your selections
- **One-Click Export** – Copy complete prompts directly to your clipboard

### 🎬 **Video & Image Modes**
- **Image Mode** – Optimized for single-frame generation (Midjourney, DALL-E, Stable Diffusion)
- **Video Mode** – Motion parameters for video generators (Sora, Runway, Pika)
- **Mode-Aware Prompts** – Automatically includes/excludes relevant parameters

### 💾 **Project Management**
- **Save/Load States** – Never lose your work with timestamped snapshots
- **Custom Presets** – Create and save your own character configurations
- **Search & Filter** – Quickly find poses, expressions, and styles from extensive libraries

### 🎯 **Professional Workflow**
- **JSON-Driven UI** – Fully customizable interface via configuration files
- **Zero Dependencies** – Self-contained executable with embedded assets
- **Native Performance** – Built in Rust using egui for instant responsiveness
- **Windows Native** – Optimized executable for Windows 10/11

---

## 🚀 Quick Start

### Installation

1. **Download** the latest release from [Releases](https://github.com/Eric-Lautanen/PromptPuppet/releases)
2. **Run** the executable – No installation required!
3. **Start Creating** – The app is fully self-contained with all assets embedded

### Basic Workflow

1. **Pose Your Character**
   - Drag joints on the canvas to create your desired pose
   - Use the preset library for common poses (standing, sitting, action poses)

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

- **Compile-Time Asset Embedding** – All JSON configs, fonts, and icons bundled via `include_str!` and `include_bytes!`
- **Zero External Files** – Single executable deployment
- **Reactive State Management** – Automatic prompt regeneration on state changes
- **Modular JSON Configuration** – Easy customization of UI panels, presets, and options

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

- [ ] **Linux & macOS Support** – Cross-platform builds (currently Windows-only)
- [ ] **3D Pose Preview** – OpenGL/WebGPU-based 3D character visualization
- [ ] **Animation Timeline** – Keyframe-based pose sequences for video
- [ ] **Pose Import** – Load from OpenPose JSON, MediaPipe, or other formats
- [ ] **Cloud Sync** – Optional online preset library and state backup
- [ ] **Plugin System** – Community-contributed presets and configurations
- [ ] **Direct API Integration** – Generate images without leaving the app

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

This project is licensed under the MIT License with Commons Clause

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software—**except** you may not sell the Software itself, or
offer any product or service that incorporates it, without explicit written
permission from the copyright holder.

---

## 🙏 Acknowledgments

- Built with [egui](https://github.com/emilk/egui) by Emil Ernerfeldt
- Inspired by pose estimation tools like OpenPose and MediaPipe
- Icon and UI design by the PromptPuppet team

---

## 📬 Contact & Support

- **GitHub Issues:** [Report bugs or request features](https://github.com/Eric-Lautanen/PromptPuppet/issues)
- **Discussions:** [Join the community](https://github.com/Eric-Lautanen/PromptPuppet/discussions)

---

## 🌟 Star History

If you find PromptPuppet useful, please consider giving it a star ⭐ on GitHub!

---

**Made with ❤️ for the AI art community**