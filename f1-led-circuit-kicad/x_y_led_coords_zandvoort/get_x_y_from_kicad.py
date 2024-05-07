import pcbnew
import csv

# Load the current board
board = pcbnew.GetBoard()

# Define the range of designators you are interested in, e.g., U1 to U96
designator_prefix = "U"
designator_range = range(1, 97) 

# Prepare a list to hold the component data
component_data = []

# Iterate over all components
for module in board.GetFootprints():
    ref = module.GetReference()
    # Check if the component's designator is in the desired range
    if ref.startswith(designator_prefix):
        try:
            designator_number = int(ref.lstrip(designator_prefix))
            if designator_number in designator_range:
                pos = module.GetPosition()
                # Convert from nanometers to millimeters
                x = pcbnew.ToMM(pos.x)
                y = pcbnew.ToMM(pos.y)
                # Get orientation, convert to degrees and handle EDA_ANGLE object
                orientation = module.GetOrientation()
                angle = orientation.AsDegrees()  # Convert EDA_ANGLE to degrees
                component_data.append({"Designator": ref, "X": x, "Y": y, "Angle": angle})
        except ValueError:
            # The designator number wasn't an integer, skip this component
            pass

# Define the CSV file path
csv_file_path = '/Users/hott/eng/f1-led-circuit/f1-led-circuit-kicad/x_y_led_coords_zandvoort/zandvoort_led_coordinates.csv'

# Write the data to a CSV file
with open(csv_file_path, mode='w', newline='') as file:
    writer = csv.DictWriter(file, fieldnames=["Designator", "X", "Y", "Angle"])
    writer.writeheader()
    for data in component_data:
        writer.writerow(data)

print(f"Data exported to {csv_file_path}")
