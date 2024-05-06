import pcbnew

# Load the current board
board = pcbnew.GetBoard()

# Loop over all footprints (components)
for footprint in board.GetFootprints():
    ref = footprint.GetReference()
    if ref.startswith("U"):
        # Extract the numeric part of the reference designator
        try:
            num = int(ref[1:])  # Assumes the format is U<number>
            if 1 <= num <= 90:
                footprint.SetSelected()  # Select the component without argument
        except ValueError:
            pass

# Refresh to update the selection visually
pcbnew.Refresh()
