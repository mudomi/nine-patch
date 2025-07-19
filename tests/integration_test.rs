use nine_patch::{nine_patch, nine_patch_content_info};
use image::{ImageBuffer, Rgba, ImageFormat};
use std::io::Cursor;

#[test]
fn test_nine_patch_basic() {
    // Create a simple 5x5 nine-patch image for testing
    // Structure:
    // [transparent][black][transparent][transparent][transparent]
    // [black][content][content][content][transparent]
    // [transparent][content][content][content][transparent] 
    // [transparent][content][content][content][transparent]
    // [transparent][transparent][transparent][transparent][transparent]
    
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(5, 5);
    
    // Fill with transparent pixels
    for y in 0..5 {
        for x in 0..5 {
            img.put_pixel(x, y, Rgba([255, 255, 255, 0])); // Transparent
        }
    }
    
    // Set stretch markers (black pixels)
    img.put_pixel(1, 0, Rgba([0, 0, 0, 255])); // Top stretch marker
    img.put_pixel(0, 1, Rgba([0, 0, 0, 255])); // Left stretch marker
    
    // Fill content area with white
    for y in 1..4 {
        for x in 1..4 {
            img.put_pixel(x, y, Rgba([255, 255, 255, 255])); // White content
        }
    }
    
    // Encode the test image as PNG
    let mut png_data = Vec::new();
    {
        let mut cursor = Cursor::new(&mut png_data);
        img.write_to(&mut cursor, ImageFormat::Png).unwrap();
    }
    
    // Test scaling to 10x8
    let target_width = 10u32;
    let target_height = 8u32;
    
    let width_bytes = target_width.to_le_bytes();
    let height_bytes = target_height.to_le_bytes();
    
    let result = nine_patch(&png_data, &width_bytes, &height_bytes);
    
    // Should not be empty (no error)
    assert!(!result.is_empty(), "Nine-patch result should not be empty");
    
    // Try to decode the result
    let result_img = image::load_from_memory(&result);
    assert!(result_img.is_ok(), "Result should be a valid image");
    
    let result_img = result_img.unwrap().to_rgba8();
    let (width, height) = result_img.dimensions();
    
    assert_eq!(width, target_width, "Result width should match target");
    assert_eq!(height, target_height, "Result height should match target");
}

#[test]
fn test_nine_patch_too_small() {
    // Create a 5x5 nine-patch with fixed corners that are 2x2 each
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(5, 5);
    
    // Fill with white
    for y in 0..5 {
        for x in 0..5 {
            img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
        }
    }
    
    // Add stretch markers for only the middle pixel
    // This creates: [fixed][fixed][stretch][fixed][fixed] 
    // So minimum width = 2 + 2 = 4 (left_fixed + right_fixed)
    img.put_pixel(2, 0, Rgba([0, 0, 0, 255])); // Top stretch marker (only middle)
    img.put_pixel(0, 2, Rgba([0, 0, 0, 255])); // Left stretch marker (only middle)
    
    // Encode as PNG
    let mut png_data = Vec::new();
    {
        let mut cursor = Cursor::new(&mut png_data);
        img.write_to(&mut cursor, ImageFormat::Png).unwrap();
    }
    
    // Try to scale to something smaller than the minimum (4x4)
    let target_width = 1u32;
    let target_height = 1u32;
    
    let width_bytes = target_width.to_le_bytes();
    let height_bytes = target_height.to_le_bytes();
    
    let result = nine_patch(&png_data, &width_bytes, &height_bytes);
    
    // Should return empty vec due to error
    assert!(result.is_empty(), "Should return empty result for target too small");
}

#[test]
fn test_nine_patch_content_info() {
    // Create a 7x7 nine-patch image with content padding markers
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(7, 7);
    
    // Fill with white
    for y in 0..7 {
        for x in 0..7 {
            img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
        }
    }
    
    // Add stretch markers on top and left (for nine-patch functionality)
    img.put_pixel(2, 0, Rgba([0, 0, 0, 255])); // Top stretch marker
    img.put_pixel(3, 0, Rgba([0, 0, 0, 255])); // Top stretch marker
    img.put_pixel(0, 2, Rgba([0, 0, 0, 255])); // Left stretch marker
    img.put_pixel(0, 3, Rgba([0, 0, 0, 255])); // Left stretch marker
    
    // Add content padding markers on bottom and right
    // Bottom edge: content spans from x=1 to x=4 (in content coordinates)
    img.put_pixel(2, 6, Rgba([0, 0, 0, 255])); // Bottom content marker
    img.put_pixel(3, 6, Rgba([0, 0, 0, 255])); // Bottom content marker
    img.put_pixel(4, 6, Rgba([0, 0, 0, 255])); // Bottom content marker
    img.put_pixel(5, 6, Rgba([0, 0, 0, 255])); // Bottom content marker
    
    // Right edge: content spans from y=1 to y=3 (in content coordinates)
    img.put_pixel(6, 2, Rgba([0, 0, 0, 255])); // Right content marker
    img.put_pixel(6, 3, Rgba([0, 0, 0, 255])); // Right content marker
    img.put_pixel(6, 4, Rgba([0, 0, 0, 255])); // Right content marker
    
    // Encode as PNG
    let mut png_data = Vec::new();
    {
        let mut cursor = Cursor::new(&mut png_data);
        img.write_to(&mut cursor, ImageFormat::Png).unwrap();
    }
    
    let result = nine_patch_content_info(&png_data);
    
    assert!(!result.is_empty(), "Content info result should not be empty");
    assert_eq!(result.len(), 24, "Result should contain 6 u32 values (24 bytes)");
    
    // Parse the result
    let content_left = u32::from_le_bytes([result[0], result[1], result[2], result[3]]);
    let content_top = u32::from_le_bytes([result[4], result[5], result[6], result[7]]);
    let content_right = u32::from_le_bytes([result[8], result[9], result[10], result[11]]);
    let content_bottom = u32::from_le_bytes([result[12], result[13], result[14], result[15]]);
    let min_width = u32::from_le_bytes([result[16], result[17], result[18], result[19]]);
    let min_height = u32::from_le_bytes([result[20], result[21], result[22], result[23]]);
    
    // Expected values based on the content markers we set
    // content_left and content_top are distances from left/top edges
    assert_eq!(content_left, 1, "Content left should be 1");
    assert_eq!(content_top, 1, "Content top should be 1"); 
    // content_right and content_bottom are distances from right/bottom edges
    // Content area is 5x5, markers span from 1 to 4, so right distance = 5-5 = 0, bottom distance = 5-4 = 1
    assert_eq!(content_right, 0, "Content right should be 0 (distance from right edge)");
    assert_eq!(content_bottom, 1, "Content bottom should be 1 (distance from bottom edge)");
    
    // Expected minimum dimensions based on stretch markers
    // Stretch markers are at positions 2,3 on top and 2,3 on left
    // In content coordinates: stretch_left=1, stretch_right=3, so left_fixed=1, right_fixed=5-3=2
    // Similarly for vertical: stretch_top=1, stretch_bottom=3, so top_fixed=1, bottom_fixed=5-3=2
    assert_eq!(min_width, 3, "Minimum width should be 3 (left_fixed + right_fixed = 1 + 2)");
    assert_eq!(min_height, 3, "Minimum height should be 3 (top_fixed + bottom_fixed = 1 + 2)");
}
