# Project Requirements

## Race Preparation
- Before the race countdown begins, 20 cars line up in a starting grid behind the starting line. We will place 20 LEDs to represent the cars in the starting grid, arranged as 2 rows of 10 LEDs each.
- Before the race starts, a 5 light countdown timer starts. The countdown timer reduces the illuminated lights from 5 to 4, then 4 to 3, and so forth until 0. When there are 0 lights present, the race starts. We will have 5 LED lights that represent the countdown timer.

## Connectivity
- We need to be able to connect to Wifi for any user's router.
- We have found several data sources that are possible candidates. We need to be able to get a dataset outside of the system and pull the data into our system.

## Team and Driver Representation
- There are 2 drivers (2 cars) for each racing team. Typically, they are represented on a virtual track as having the same color. However, virtual tracks also have a text field that represents the driver's name, making the 2 drivers from different teams distinguishable. We must have a method to distinguish drivers on the same team for the LED lights that represent them.

## Handling Race Incidents
- There are several scenarios when a car is out of the race, such as a car crash or the driver being disqualified. We must handle the scenario where a car is out of the race.

## Safety and Reliability
- There is a possibility that using possibly 500+ LED lights on the board could, due to user customization or system failure, cause all LEDs on the board to light up white at the same time, pulling the maximum amount of current into the board. This could destroy the board. We must have a method for the board to shut down if a maximum temperature is reached.
- The board must pass standard EMC/ESD tests.

## User Interface
- The user must have a way for the system to be turned on and off. We must have an On/Off Button for the user to do this.
- The user must have a way for the system to start the countdown timer for the race to begin. Conversely, the user must also be able to stop the race while it's running. We must have a Start/Stop Button for the user to do this.
- We must show the visual state of the program, whether the program is running or not.

## Firmware and Hardware
- The firmware should know its own version.
- The firmware should be able to detect PCB version.
- The board must be debuggable with RTT.
- The board must be powered. A possible candidate is a wall outlet to USB-C adapter.

## LED Driver
- We must ensure that the LED driver provides the correct voltage and current to the LEDs. Overvoltage or excessive current can lead to overheating, potentially causing fires or damaging the LEDs. We must protect the board from that. We must write an LED driver that is safe.
