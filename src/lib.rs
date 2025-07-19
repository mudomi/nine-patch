use wasm_minimal_protocol::*;
use image::{ImageBuffer, Rgba, RgbaImage, ImageFormat};
use std::io::Cursor;

initiate_protocol!();

#[derive(Debug)]
pub enum NinePatchError {
    InvalidImage(String),
    TargetTooSmall(String),
    InvalidFormat(String),
}

impl std::fmt::Display for NinePatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NinePatchError::InvalidImage(msg) => write!(f, "Invalid image: {}", msg),
            NinePatchError::TargetTooSmall(msg) => write!(f, "Target size too small: {}", msg),
            NinePatchError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
        }
    }
}

impl std::error::Error for NinePatchError {}

#[wasm_func]
pub fn nine_patch(
    image_bytes: &[u8],
    width: &[u8],
    height: &[u8],
) -> Vec<u8> {
    // Parse target dimensions
    let target_width = u32::from_le_bytes([width[0], width[1], width[2], width[3]]);
    let target_height = u32::from_le_bytes([height[0], height[1], height[2], height[3]]);
    
    match nine_patch_impl(image_bytes, target_width, target_height) {
        Ok(result) => result,
        Err(e) => {
            // Return empty vec on error - in a real implementation you might want better error handling
            eprintln!("Nine-patch error: {}", e);
            Vec::new()
        }
    }
}

fn nine_patch_impl(image_bytes: &[u8], target_width: u32, target_height: u32) -> Result<Vec<u8>, NinePatchError> {
    // Load the image
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| NinePatchError::InvalidImage(format!("Failed to load image: {}", e)))?;
    
    let rgba_img = img.to_rgba8();
    let (orig_width, orig_height) = rgba_img.dimensions();
    
    if orig_width < 3 || orig_height < 3 {
        return Err(NinePatchError::InvalidImage("Image too small for nine-patch".to_string()));
    }
    
    // Parse nine-patch metadata from border pixels
    let stretch_info = parse_nine_patch_borders(&rgba_img)?;
    
    // Calculate minimum required size
    let min_width = stretch_info.left_fixed + stretch_info.right_fixed;
    let min_height = stretch_info.top_fixed + stretch_info.bottom_fixed;
    
    if target_width < min_width || target_height < min_height {
        return Err(NinePatchError::TargetTooSmall(
            format!("Target size {}x{} is smaller than minimum {}x{}", 
                   target_width, target_height, min_width, min_height)
        ));
    }
    
    // Remove the outer border pixels to get the actual content
    let content_img = extract_content(&rgba_img);
    
    // Create the scaled nine-patch image
    let result_img = scale_nine_patch(&content_img, &stretch_info, target_width, target_height)?;
    
    // Encode as PNG
    let mut buffer = Vec::new();
    {
        let mut cursor = Cursor::new(&mut buffer);
        result_img.write_to(&mut cursor, ImageFormat::Png)
            .map_err(|e| NinePatchError::InvalidFormat(format!("Failed to encode PNG: {}", e)))?;
    }
    
    Ok(buffer)
}

#[derive(Debug)]
struct StretchInfo {
    left_fixed: u32,
    right_fixed: u32,
    top_fixed: u32,
    bottom_fixed: u32,
    stretch_left: u32,
    stretch_right: u32,
    stretch_top: u32,
    stretch_bottom: u32,
}

fn parse_nine_patch_borders(img: &RgbaImage) -> Result<StretchInfo, NinePatchError> {
    let (width, height) = img.dimensions();
    
    // Parse horizontal stretch regions from top border
    let (stretch_left, stretch_right) = parse_stretch_line(img, 0, width, true)?;
    
    // Parse vertical stretch regions from left border  
    let (stretch_top, stretch_bottom) = parse_stretch_line(img, 0, height, false)?;
    
    let content_width = width - 2; // Remove left and right borders
    let content_height = height - 2; // Remove top and bottom borders
    
    let left_fixed = stretch_left;
    let right_fixed = content_width - stretch_right;
    let top_fixed = stretch_top;
    let bottom_fixed = content_height - stretch_bottom;
    
    Ok(StretchInfo {
        left_fixed,
        right_fixed, 
        top_fixed,
        bottom_fixed,
        stretch_left,
        stretch_right,
        stretch_top,
        stretch_bottom,
    })
}

fn parse_stretch_line(img: &RgbaImage, coord: u32, length: u32, horizontal: bool) -> Result<(u32, u32), NinePatchError> {
    let black = Rgba([0, 0, 0, 255]);
    
    let mut stretch_start = None;
    let mut stretch_end = None;
    
    // Skip first and last pixels (corners)
    for i in 1..length-1 {
        let pixel = if horizontal {
            *img.get_pixel(i, coord)
        } else {
            *img.get_pixel(coord, i)
        };
        
        if pixel == black {
            if stretch_start.is_none() {
                stretch_start = Some(i - 1); // Convert to content coordinates
            }
            stretch_end = Some(i - 1); // Convert to content coordinates
        }
    }
    
    match (stretch_start, stretch_end) {
        (Some(start), Some(end)) => Ok((start, end + 1)),
        _ => {
            // If no stretch markers, treat entire content as non-stretchable
            // Return (content_length, content_length) to indicate no stretch region
            let content_length = length - 2;
            Ok((content_length, content_length))
        }
    }
}

fn extract_content(img: &RgbaImage) -> RgbaImage {
    let (width, height) = img.dimensions();
    let content_width = width - 2;
    let content_height = height - 2;
    
    let mut content = ImageBuffer::new(content_width, content_height);
    
    for y in 0..content_height {
        for x in 0..content_width {
            let pixel = *img.get_pixel(x + 1, y + 1);
            content.put_pixel(x, y, pixel);
        }
    }
    
    content
}

fn scale_nine_patch(
    content: &RgbaImage,
    stretch_info: &StretchInfo,
    target_width: u32,
    target_height: u32,
) -> Result<RgbaImage, NinePatchError> {
    let (_content_width, _content_height) = content.dimensions();
    
    // Calculate stretch amounts
    let extra_width = target_width - (stretch_info.left_fixed + stretch_info.right_fixed);
    let extra_height = target_height - (stretch_info.top_fixed + stretch_info.bottom_fixed);
    
    let mut result = ImageBuffer::new(target_width, target_height);
    
    // Copy/scale the 9 patches
    
    // Top-left corner (fixed)
    copy_region(content, &mut result, 
               0, 0, stretch_info.left_fixed, stretch_info.top_fixed,
               0, 0);
    
    // Top edge (stretch horizontally)
    let top_stretch_width = stretch_info.stretch_right - stretch_info.stretch_left;
    if top_stretch_width > 0 {
        let top_section = extract_region(content, stretch_info.stretch_left, 0, top_stretch_width, stretch_info.top_fixed);
        let scaled_top = resize_image(&top_section, extra_width, stretch_info.top_fixed);
        copy_image(&scaled_top, &mut result, stretch_info.left_fixed, 0);
    }
    
    // Top-right corner (fixed)
    copy_region(content, &mut result,
               stretch_info.stretch_right, 0, stretch_info.right_fixed, stretch_info.top_fixed,
               stretch_info.left_fixed + extra_width, 0);
    
    // Left edge (stretch vertically)
    let left_stretch_height = stretch_info.stretch_bottom - stretch_info.stretch_top;
    if left_stretch_height > 0 {
        let left_section = extract_region(content, 0, stretch_info.stretch_top, stretch_info.left_fixed, left_stretch_height);
        let scaled_left = resize_image(&left_section, stretch_info.left_fixed, extra_height);
        copy_image(&scaled_left, &mut result, 0, stretch_info.top_fixed);
    }
    
    // Center (stretch both ways)
    if top_stretch_width > 0 && left_stretch_height > 0 {
        let center_section = extract_region(content, stretch_info.stretch_left, stretch_info.stretch_top, top_stretch_width, left_stretch_height);
        let scaled_center = resize_image(&center_section, extra_width, extra_height);
        copy_image(&scaled_center, &mut result, stretch_info.left_fixed, stretch_info.top_fixed);
    }
    
    // Right edge (stretch vertically)
    if left_stretch_height > 0 {
        let right_section = extract_region(content, stretch_info.stretch_right, stretch_info.stretch_top, stretch_info.right_fixed, left_stretch_height);
        let scaled_right = resize_image(&right_section, stretch_info.right_fixed, extra_height);
        copy_image(&scaled_right, &mut result, stretch_info.left_fixed + extra_width, stretch_info.top_fixed);
    }
    
    // Bottom-left corner (fixed)
    copy_region(content, &mut result,
               0, stretch_info.stretch_bottom, stretch_info.left_fixed, stretch_info.bottom_fixed,
               0, stretch_info.top_fixed + extra_height);
    
    // Bottom edge (stretch horizontally)
    if top_stretch_width > 0 {
        let bottom_section = extract_region(content, stretch_info.stretch_left, stretch_info.stretch_bottom, top_stretch_width, stretch_info.bottom_fixed);
        let scaled_bottom = resize_image(&bottom_section, extra_width, stretch_info.bottom_fixed);
        copy_image(&scaled_bottom, &mut result, stretch_info.left_fixed, stretch_info.top_fixed + extra_height);
    }
    
    // Bottom-right corner (fixed)
    copy_region(content, &mut result,
               stretch_info.stretch_right, stretch_info.stretch_bottom, stretch_info.right_fixed, stretch_info.bottom_fixed,
               stretch_info.left_fixed + extra_width, stretch_info.top_fixed + extra_height);
    
    Ok(result)
}

fn extract_region(img: &RgbaImage, x: u32, y: u32, width: u32, height: u32) -> RgbaImage {
    let mut region = ImageBuffer::new(width, height);
    
    for dy in 0..height {
        for dx in 0..width {
            if x + dx < img.width() && y + dy < img.height() {
                let pixel = *img.get_pixel(x + dx, y + dy);
                region.put_pixel(dx, dy, pixel);
            }
        }
    }
    
    region
}

fn copy_region(src: &RgbaImage, dst: &mut RgbaImage, src_x: u32, src_y: u32, width: u32, height: u32, dst_x: u32, dst_y: u32) {
    for dy in 0..height {
        for dx in 0..width {
            if src_x + dx < src.width() && src_y + dy < src.height() && 
               dst_x + dx < dst.width() && dst_y + dy < dst.height() {
                let pixel = *src.get_pixel(src_x + dx, src_y + dy);
                dst.put_pixel(dst_x + dx, dst_y + dy, pixel);
            }
        }
    }
}

fn copy_image(src: &RgbaImage, dst: &mut RgbaImage, dst_x: u32, dst_y: u32) {
    let (src_width, src_height) = src.dimensions();
    copy_region(src, dst, 0, 0, src_width, src_height, dst_x, dst_y);
}

// Simple nearest-neighbor image resize
fn resize_image(src: &RgbaImage, new_width: u32, new_height: u32) -> RgbaImage {
    let (src_width, src_height) = src.dimensions();
    let mut dst = ImageBuffer::new(new_width, new_height);
    
    for y in 0..new_height {
        for x in 0..new_width {
            // Map destination coordinates to source coordinates
            let src_x = (x * src_width) / new_width;
            let src_y = (y * src_height) / new_height;
            
            // Clamp to ensure we don't go out of bounds
            let src_x = src_x.min(src_width - 1);
            let src_y = src_y.min(src_height - 1);
            
            let pixel = *src.get_pixel(src_x, src_y);
            dst.put_pixel(x, y, pixel);
        }
    }
    
    dst
}

#[wasm_func]
pub fn nine_patch_content_info(
    image_bytes: &[u8],
) -> Vec<u8> {
    match nine_patch_content_info_impl(image_bytes) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Nine-patch content info error: {}", e);
            Vec::new()
        }
    }
}

fn nine_patch_content_info_impl(image_bytes: &[u8]) -> Result<Vec<u8>, NinePatchError> {
    // Load the image
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| NinePatchError::InvalidImage(format!("Failed to load image: {}", e)))?;
    
    let rgba_img = img.to_rgba8();
    let (orig_width, orig_height) = rgba_img.dimensions();
    
    if orig_width < 3 || orig_height < 3 {
        return Err(NinePatchError::InvalidImage("Image too small for nine-patch".to_string()));
    }
    
    // Parse content padding from right and bottom borders
    let content_info = parse_content_borders(&rgba_img)?;
    
    // Parse stretch info to calculate minimum dimensions
    let stretch_info = parse_nine_patch_borders(&rgba_img)?;
    let min_width = stretch_info.left_fixed + stretch_info.right_fixed;
    let min_height = stretch_info.top_fixed + stretch_info.bottom_fixed;
    
    // Return as bytes: [content_left, content_top, content_right, content_bottom, min_width, min_height] as u32 little-endian
    let mut result = Vec::new();
    result.extend_from_slice(&content_info.content_left.to_le_bytes());
    result.extend_from_slice(&content_info.content_top.to_le_bytes());
    result.extend_from_slice(&content_info.content_right.to_le_bytes());
    result.extend_from_slice(&content_info.content_bottom.to_le_bytes());
    result.extend_from_slice(&min_width.to_le_bytes());
    result.extend_from_slice(&min_height.to_le_bytes());
    
    Ok(result)
}

#[derive(Debug)]
struct ContentInfo {
    content_left: u32,
    content_top: u32,
    content_right: u32,
    content_bottom: u32,
}

fn parse_content_borders(img: &RgbaImage) -> Result<ContentInfo, NinePatchError> {
    let (width, height) = img.dimensions();
    
    // Parse horizontal content region from bottom border (row height-1)
    let (content_left, content_right) = parse_content_line(img, height - 1, width, true)?;
    
    // Parse vertical content region from right border (column width-1)
    let (content_top, content_bottom) = parse_content_line(img, width - 1, height, false)?;
    
    let content_width = width - 2; // Remove left and right borders
    let content_height = height - 2; // Remove top and bottom borders
    
    Ok(ContentInfo {
        content_left,
        content_top,
        // Make right and bottom relative to their respective edges
        content_right: content_width - content_right,
        content_bottom: content_height - content_bottom,
    })
}

fn parse_content_line(img: &RgbaImage, coord: u32, length: u32, horizontal: bool) -> Result<(u32, u32), NinePatchError> {
    let black = Rgba([0, 0, 0, 255]);
    
    let mut content_start = None;
    let mut content_end = None;
    
    // Skip first and last pixels (corners)
    for i in 1..length-1 {
        let pixel = if horizontal {
            *img.get_pixel(i, coord)
        } else {
            *img.get_pixel(coord, i)
        };
        
        if pixel == black {
            if content_start.is_none() {
                content_start = Some(i - 1); // Convert to content coordinates
            }
            content_end = Some(i - 1); // Convert to content coordinates
        }
    }
    
    match (content_start, content_end) {
        (Some(start), Some(end)) => Ok((start, end + 1)),
        _ => {
            // If no content markers, use the entire content area
            Ok((0, length - 2))
        }
    }
}
