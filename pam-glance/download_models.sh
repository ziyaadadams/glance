#!/bin/bash
# Download dlib model files for face recognition

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}   Download dlib Models${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

MODELS_DIR="/usr/share/facerec/models"

# Check for root (needed to write to /usr/share)
if [ "$EUID" -ne 0 ]; then
    echo -e "${YELLOW}Note: Running without root, will download to current directory${NC}"
    MODELS_DIR="./models"
fi

mkdir -p "$MODELS_DIR"
cd "$MODELS_DIR"

# Shape predictor (68 landmarks)
SHAPE_PREDICTOR="shape_predictor_68_face_landmarks.dat"
if [ ! -f "$SHAPE_PREDICTOR" ]; then
    echo -e "${YELLOW}Downloading shape predictor...${NC}"
    wget -q --show-progress "http://dlib.net/files/${SHAPE_PREDICTOR}.bz2"
    echo "Extracting..."
    bunzip2 "${SHAPE_PREDICTOR}.bz2"
    echo -e "${GREEN}✓ Shape predictor downloaded${NC}"
else
    echo -e "${GREEN}✓ Shape predictor already exists${NC}"
fi

# Face recognition model
FACE_REC_MODEL="dlib_face_recognition_resnet_model_v1.dat"
if [ ! -f "$FACE_REC_MODEL" ]; then
    echo -e "${YELLOW}Downloading face recognition model...${NC}"
    wget -q --show-progress "http://dlib.net/files/${FACE_REC_MODEL}.bz2"
    echo "Extracting..."
    bunzip2 "${FACE_REC_MODEL}.bz2"
    echo -e "${GREEN}✓ Face recognition model downloaded${NC}"
else
    echo -e "${GREEN}✓ Face recognition model already exists${NC}"
fi

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}   Models Downloaded!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Models location: $MODELS_DIR"
echo ""
ls -lh "$MODELS_DIR"
echo ""

if [ "$MODELS_DIR" = "./models" ]; then
    echo -e "${YELLOW}To install system-wide, run as root:${NC}"
    echo "  sudo mkdir -p /usr/share/facerec/models"
    echo "  sudo cp models/* /usr/share/facerec/models/"
    echo ""
fi
