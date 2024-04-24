### F1-LED-CIRCUIT (Work In Progress)

## Instructions

### Step 1: Clone the Repository
  1. Use Git to clone this repository to your local machine.
  ```bash
  git clone https://github.com/JitterCompany/f1-led-circuit
  ```
  
  2. Navitage to the cloned directory
  3. Navigate to the f1-led-circuit-kicad directory
  4. Open file f1-led-circuit.kicad_pro from the KiCad application

### Step 2: Add External Symbol Libraries to KiCad
Symbols represent electronic components in your schematic. 
You need to add project-specific symbol libraries to KiCad.

Jitter Symbols - Use Git to clone this repository to your local machine.
```bash
git clone https://github.com/JitterCompany/KicadComponents.git
```

  1. Add the library to KiCad:
  2. Open KiCad and go to "Preferences" > "Manage Symbol Libraries".
  3. Select "Project Libraries" if you want the symbols only for this project, or "Global Libraries" to make them available for all projects.
  4. Click "Add Library" and navigate to the cloned KicadComponents directory.
  5. Select the appropriate .lib file and click "Open".

ESP32 Symbols - Use the Plugin and Content Manager in KiCad
  
  1. Navigate to this this repository - https://github.com/espressif/kicad-libraries
  2. Follow the instructions in its README to add the library using the Plugin and Content Manager (PCM) in KiCad.

### Step 3: Add Custom Project Symbols Libraries to KiCad
  1. In the cloned repository, navigate to /symbols.
  2. Open KiCad and select "Schematics" to enter the Schematics tool.
  3. Go to "Preferences" > "Manage Symbol Libraries".
  4. Select "Project Specific Libraries" at the top.
  5. Click the "+" button at the bottom left.
  6. Add the following information to the grid.

  Nickname: F1-LED-CIRCUIT-LIBRARY
  Library Path: ${KIPRJMOD}/symbols/F1-LED-CIRCUIT-LIBRARY.kicad_sym

  Nickname: adams_library_symbols
  Library Path: ${KIPRJMOD}/symbols/adams_library_symbols.kicad_sym

  7. Click Ok
  
### Step 4: Add Custom Project Footprint Libraries to KiCad

  1. In the cloned repository, navigate to /footprints/F1-LED-CIRCUIT-LIBRARY.pretty.
  2. Open KiCad and select "Pcbnew" to enter the PCB layout tool.
  3. Go to "Preferences" > "Manage Footprint Libraries."
  4. Select "Project Libraries" to add the footprints to this project or "Global Libraries" for all projects.
  5. Click "Add Library" and navigate to the F1-LED-CIRCUIT-LIBRARY.pretty folder.
  6. Select the folder and click "OK" to add it to your KiCad project.

