### F1-LED-CIRCUIT (WIP)

## Instructions

### Step 1: Clone the Repository
- Use Git to clone this repository to your local machine.
  git clone https://github.com/JitterCompany/f1-led-circuit
- Navitage to the cloned directory:
  cd F1-LED-CIRCUIT

### Step 2: Add Symbol Libraries to KiCad
Symbols represent electronic components in your schematic. 
You need to add project-specific symbol libraries to KiCad.

- Jitter Symbols - Use Git to clone this repository to your local machine.
  git clone https://github.com/JitterCompany/KicadComponents.git

  1. Add the library to KiCad:
  2. Open KiCad and go to "Preferences" > "Manage Symbol Libraries."
  3. Select "Project Libraries" if you want the symbols only for this project, or "Global Libraries" to make them available for all projects.
  4. Click "Add Library" and navigate to the cloned KicadComponents directory.
  5. Select the appropriate .lib file and click "Open".

- ESP32 Symbols - Use the Plugin and Content Manager in KiCad
  
  1. Navigate to this this repository - https://github.com/espressif/kicad-libraries
  2. Follow the instructions in its README to add the library using the Plugin and Content Manager (PCM) in KiCad.
  
### Step 3: Add Project Footprint Libraries to KiCad

  1. In the cloned repository, navigate to /footprints/F1-LED-CIRCUIT-LIBRARY.pretty.
  2. Open KiCad and select "Pcbnew" to enter the PCB layout tool.
  3. Go to "Preferences" > "Manage Footprint Libraries."
  4. Select "Project Libraries" to add the footprints to this project or "Global Libraries" for all projects.
  5. Click "Add Library" and navigate to the F1-LED-CIRCUIT-LIBRARY.pretty folder.
  6. Select the folder and click "OK" to add it to your KiCad project.

