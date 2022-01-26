use yuv::YUV;

/// Iterator that combines equal-sized planes of Y, U, V into YUV pixels
pub fn yuv_444<'a, T: Copy + 'a, YRowsIter: 'a, URowsIter: 'a, VRowsIter: 'a>(y: YRowsIter, u: URowsIter, v: VRowsIter) -> impl Iterator<Item = YUV<T>> + 'a
    where YRowsIter: Iterator<Item=&'a [T]>,
        URowsIter: Iterator<Item=&'a [T]>,
        VRowsIter: Iterator<Item=&'a [T]>
{
    y.zip(u.zip(v))
        .flat_map(|(y,(u,v))| {
            y.iter().copied().zip(u.iter().copied().zip(v.iter().copied()))
            .map(|(y,(u,v))| YUV{y,u,v})
        })
}

/// Iterator that combines planes of Y, U, V into YUV pixels, where U and V have half width
///
/// Uses nearest-neighbor scaling.
pub fn yuv_422<'a, T: Copy + 'a, YRowsIter: 'a, URowsIter: 'a, VRowsIter: 'a>(y: YRowsIter, u: URowsIter, v: VRowsIter) -> impl Iterator<Item = YUV<T>> + 'a
    where YRowsIter: Iterator<Item=&'a [T]>,
        URowsIter: Iterator<Item=&'a [T]>,
        VRowsIter: Iterator<Item=&'a [T]>
{
    y.zip(u.zip(v))
        .flat_map(|(y,(u,v))| {
            let u = u.iter().copied().flat_map(|u_px| std::iter::repeat(u_px).take(2));
            let v = v.iter().copied().flat_map(|v_px| std::iter::repeat(v_px).take(2));
            y.iter().copied().zip(u.zip(v))
            .map(|(y,(u,v))| YUV{y,u,v})
        })
}

/// Iterator that combines planes of Y, U, V into YUV pixels, where U and V have half width and half height
///
/// Uses nearest-neighbor scaling.
pub fn yuv_420<'a, T: Copy + 'a, YRowsIter: 'a, URowsIter: 'a, VRowsIter: 'a>(y: YRowsIter, u: URowsIter, v: VRowsIter) -> impl Iterator<Item = YUV<T>> + 'a
    where YRowsIter: Iterator<Item=&'a [T]>,
        URowsIter: Iterator<Item=&'a [T]>,
        VRowsIter: Iterator<Item=&'a [T]>
{
    let u = u.flat_map(|u_row| std::iter::repeat(u_row).take(2));
    let v = v.flat_map(|v_row| std::iter::repeat(v_row).take(2));
    y.zip(u.zip(v))
    .flat_map(|(y,(u,v))| {
        let u = u.iter().copied().flat_map(|u_px| std::iter::repeat(u_px).take(2));
        let v = v.iter().copied().flat_map(|v_px| std::iter::repeat(v_px).take(2));
        y.iter().copied().zip(u.zip(v))
        .map(|(y,(u,v))| YUV{y,u,v})
    })
}
