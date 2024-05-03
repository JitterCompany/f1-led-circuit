import pcbnew
import csv

# Load the board
board = pcbnew.GetBoard()

# Path to the CSV file with coordinates
csv_file_path = '/Users/hott/eng/f1-led-circuit/f1-led-circuit-kicad/x_y_led_coords_zandvoort/zandvoort_led_coordinates.csv'

# Read the CSV file
with open(csv_file_path, newline='') as csvfile:
    reader = csv.reader(csvfile)
    next(reader)  # Skip the header row
    for row in reader:
        designator, x, y = row
        x = float(x) * 1000000  # Convert mm to nm (nanometers) as required by KiCad
        y = float(y) * 1000000
        
        # Find the footprint by reference
        module = board.FindFootprintByReference(designator)
        if module:
            # Set the new position using VECTOR2I
            module.SetPosition(pcbnew.VECTOR2I(int(x), int(y)))
        else:
            print(f"Footprint {designator} not found")

    # Save the board with changes
    board.Save('/Users/hott/eng/f1-led-circuit/f1-led-circuit-kicad/f1-led-circuit.kicad_pcb')
